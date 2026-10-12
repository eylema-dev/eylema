use eylema_runtime::resource::{
    Cancellation, ResourceClass, ResourceError, ResourceLimits, ResourceManager,
};
use std::hint::black_box;
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

fn limits() -> ResourceLimits {
    ResourceLimits {
        queued: 6,
        queued_per_class: 2,
        active: 3,
        active_per_class: [1, 1, 1],
        reserved_bytes: 192,
        reserved_bytes_per_class: [96, 96, 96],
        request_bytes: 64,
    }
}

#[test]
fn limits_reject_zero_or_inconsistent_values() {
    let mut config = limits();
    config.queued = 0;
    assert_eq!(
        ResourceManager::new(config).err(),
        Some(ResourceError::InvalidLimits)
    );
    let mut config = limits();
    config.queued_per_class = config.queued + 1;
    assert_eq!(
        ResourceManager::new(config).err(),
        Some(ResourceError::InvalidLimits)
    );
    let mut config = limits();
    config.request_bytes = config.reserved_bytes + 1;
    assert_eq!(
        ResourceManager::new(config).err(),
        Some(ResourceError::InvalidLimits)
    );
}

#[test]
fn invalid_full_and_oversized_admission_leave_all_counters_unchanged() {
    let manager = ResourceManager::new(limits()).unwrap();
    assert_eq!(
        manager.admit(ResourceClass::Metal, 0),
        Err(ResourceError::InvalidRequest)
    );
    assert_eq!(
        manager.admit(ResourceClass::Metal, 65),
        Err(ResourceError::InvalidRequest)
    );
    let first = manager.admit(ResourceClass::Metal, 32).unwrap();
    let second = manager.admit(ResourceClass::Metal, 32).unwrap();
    let before = manager.snapshot().unwrap();
    assert_eq!(
        manager.admit(ResourceClass::Metal, 1),
        Err(ResourceError::CapacityExceeded)
    );
    assert_eq!(manager.snapshot().unwrap(), before);
    assert_eq!(before.queued, 2);
    assert_eq!(before.reserved_bytes, 64);
    assert_eq!(manager.cancel(first).unwrap(), Cancellation::Effective);
    assert_eq!(manager.cancel(second).unwrap(), Cancellation::Effective);
    assert_eq!(manager.snapshot().unwrap(), Default::default());
}

#[test]
fn byte_budget_is_reserved_before_dispatch_and_released_on_drop() {
    let manager = ResourceManager::new(limits()).unwrap();
    let metal = manager.admit(ResourceClass::Metal, 64).unwrap();
    assert_eq!(
        manager.admit(ResourceClass::Metal, 33),
        Err(ResourceError::CapacityExceeded)
    );
    let lease = manager.dispatch_next().unwrap().unwrap();
    assert_eq!(lease.id(), metal);
    assert_eq!(manager.snapshot().unwrap().reserved_bytes, 64);
    drop(lease);
    assert_eq!(manager.snapshot().unwrap(), Default::default());
}

#[test]
fn total_queue_and_byte_limits_are_atomic_across_classes() {
    let mut config = limits();
    config.queued = 2;
    config.queued_per_class = 2;
    config.reserved_bytes = 512;
    config.reserved_bytes_per_class = [512; 3];
    config.request_bytes = 64;
    let manager = ResourceManager::new(config).unwrap();
    manager.admit(ResourceClass::Metal, 64).unwrap();
    manager.admit(ResourceClass::View, 64).unwrap();
    let full = manager.snapshot().unwrap();
    assert_eq!(
        manager.admit(ResourceClass::Model, 1),
        Err(ResourceError::CapacityExceeded)
    );
    assert_eq!(manager.snapshot().unwrap(), full);

    let mut config = limits();
    config.reserved_bytes = 128;
    config.reserved_bytes_per_class = [128; 3];
    config.request_bytes = 64;
    let manager = ResourceManager::new(config).unwrap();
    manager.admit(ResourceClass::Metal, 64).unwrap();
    manager.admit(ResourceClass::View, 64).unwrap();
    let full = manager.snapshot().unwrap();
    assert_eq!(
        manager.admit(ResourceClass::Model, 1),
        Err(ResourceError::CapacityExceeded)
    );
    assert_eq!(manager.snapshot().unwrap(), full);
}

#[test]
fn checked_byte_overflow_fails_without_mutating_reservations() {
    let mut config = limits();
    config.reserved_bytes = usize::MAX;
    config.reserved_bytes_per_class = [usize::MAX; 3];
    config.request_bytes = usize::MAX;
    let manager = ResourceManager::new(config).unwrap();
    manager.admit(ResourceClass::Metal, usize::MAX).unwrap();
    let before = manager.snapshot().unwrap();
    assert_eq!(
        manager.admit(ResourceClass::View, 1),
        Err(ResourceError::CapacityExceeded)
    );
    assert_eq!(manager.snapshot().unwrap(), before);
}

#[test]
fn dispatch_rotates_classes_and_keeps_fifo_within_each_class() {
    let manager = ResourceManager::new(limits()).unwrap();
    let model = manager.admit(ResourceClass::Model, 8).unwrap();
    let metal_first = manager.admit(ResourceClass::Metal, 8).unwrap();
    let metal_second = manager.admit(ResourceClass::Metal, 8).unwrap();
    let view = manager.admit(ResourceClass::View, 8).unwrap();
    let first = manager.dispatch_next().unwrap().unwrap();
    let second = manager.dispatch_next().unwrap().unwrap();
    let third = manager.dispatch_next().unwrap().unwrap();
    assert_eq!(
        (first.id(), first.class()),
        (metal_first, ResourceClass::Metal)
    );
    assert_eq!((second.id(), second.class()), (view, ResourceClass::View));
    assert_eq!((third.id(), third.class()), (model, ResourceClass::Model));
    drop((second, third));
    assert!(manager.dispatch_next().unwrap().is_none());
    drop(first);
    let next = manager.dispatch_next().unwrap().unwrap();
    assert_eq!(next.id(), metal_second);
}

#[test]
fn active_cancellation_is_cooperative_and_keeps_reservation_until_release() {
    let manager = ResourceManager::new(limits()).unwrap();
    let id = manager.admit(ResourceClass::View, 16).unwrap();
    let lease = manager.dispatch_next().unwrap().unwrap();
    assert_eq!(manager.cancel(id).unwrap(), Cancellation::Requested);
    assert_eq!(lease.check_cancelled(), Err(ResourceError::Cancelled));
    assert_eq!(manager.snapshot().unwrap().active, 1);
    assert_eq!(manager.snapshot().unwrap().reserved_bytes, 16);
    lease.finish();
    assert_eq!(manager.snapshot().unwrap(), Default::default());
    assert_eq!(manager.cancel(id), Err(ResourceError::UnknownRequest));
}

#[test]
fn combined_class_pressure_stays_bounded_and_stop_latency_is_measured() {
    let manager = ResourceManager::new(limits()).unwrap();
    for class in [
        ResourceClass::Metal,
        ResourceClass::View,
        ResourceClass::Model,
    ] {
        for _ in 0..2 {
            manager.admit(class, 16).unwrap();
        }
    }
    let mut leases = Vec::new();
    for _ in 0..3 {
        leases.push(manager.dispatch_next().unwrap().unwrap());
    }
    let snapshot = manager.snapshot().unwrap();
    assert_eq!(snapshot.queued, 3);
    assert_eq!(snapshot.active, 3);
    assert_eq!(snapshot.reserved_bytes, 96);
    for class in [
        ResourceClass::Metal,
        ResourceClass::View,
        ResourceClass::Model,
    ] {
        assert_eq!(snapshot.class(class).queued, 1);
        assert_eq!(snapshot.class(class).active, 1);
        assert!(
            snapshot.class(class).reserved_bytes
                <= limits().reserved_bytes_per_class[class as usize]
        );
    }

    let barrier = Arc::new(Barrier::new(leases.len() + 1));
    let requested_at = Arc::new(Mutex::new(None::<Instant>));
    let (done_tx, done_rx) = mpsc::channel();
    let active_ids: Vec<_> = leases.iter().map(|lease| lease.id()).collect();
    let mut workers = Vec::new();
    for lease in leases {
        let barrier = barrier.clone();
        let requested_at = requested_at.clone();
        let done_tx = done_tx.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            let mut checksum = 0_u64;
            loop {
                for n in 0..256 {
                    checksum = checksum.wrapping_add(black_box(n));
                }
                if lease.cancellation_requested() {
                    let start = requested_at
                        .lock()
                        .unwrap()
                        .expect("request timestamp is set before cancellation");
                    let elapsed = start.elapsed();
                    done_tx.send(elapsed).unwrap();
                    black_box(checksum);
                    break;
                }
            }
            drop(lease);
        }));
    }
    drop(done_tx);
    barrier.wait();
    let start = Instant::now();
    *requested_at.lock().unwrap() = Some(start);
    for id in active_ids {
        assert_eq!(manager.cancel(id).unwrap(), Cancellation::Requested);
    }
    let mut latency = Vec::new();
    for _ in 0..3 {
        latency.push(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("cooperative stop observed"),
        );
    }
    for worker in workers {
        worker.join().unwrap();
    }
    latency.sort();
    let percentile = |p: usize| latency[((latency.len() - 1) * p).div_ceil(100)];
    eprintln!(
        "resource cancellation latency under combined class pressure: p50={:?} p95={:?} max={:?}",
        percentile(50),
        percentile(95),
        latency[latency.len() - 1]
    );
    assert_eq!(manager.snapshot().unwrap().active, 0);
    assert_eq!(manager.snapshot().unwrap().reserved_bytes, 48);
    for id in [2, 4, 6] {
        assert_eq!(manager.cancel(id).unwrap(), Cancellation::Effective);
    }
    assert_eq!(manager.snapshot().unwrap(), Default::default());
}

#[test]
fn concurrent_admission_never_exceeds_global_bounds() {
    let manager = ResourceManager::new(limits()).unwrap();
    let high_water = Arc::new(Mutex::new((0usize, 0usize, 0usize)));
    let barrier = Arc::new(Barrier::new(7));
    let mut threads = Vec::new();
    for worker in 0..6 {
        let manager = manager.clone();
        let high_water = high_water.clone();
        let barrier = barrier.clone();
        threads.push(thread::spawn(move || {
            barrier.wait();
            let classes = [
                ResourceClass::Metal,
                ResourceClass::View,
                ResourceClass::Model,
            ];
            for turn in 0..300 {
                let class = classes[(worker + turn) % classes.len()];
                if let Ok(id) = manager.admit(class, 8) {
                    if let Some(lease) = manager.dispatch_next().unwrap() {
                        drop(lease);
                    } else {
                        assert!(matches!(
                            manager.cancel(id),
                            Ok(_) | Err(ResourceError::UnknownRequest)
                        ));
                    }
                }
                let snapshot = manager.snapshot().unwrap();
                assert!(snapshot.queued <= limits().queued);
                assert!(snapshot.active <= limits().active);
                assert!(snapshot.reserved_bytes <= limits().reserved_bytes);
                let mut peak = high_water.lock().unwrap();
                peak.0 = peak.0.max(snapshot.queued);
                peak.1 = peak.1.max(snapshot.active);
                peak.2 = peak.2.max(snapshot.reserved_bytes);
            }
        }));
    }
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    while let Some(lease) = manager.dispatch_next().unwrap() {
        drop(lease);
    }
    let peak = *high_water.lock().unwrap();
    eprintln!(
        "resource stress high-water: queued={}, active={}, reserved_bytes={}",
        peak.0, peak.1, peak.2
    );
    assert!(peak.0 > 0 && peak.1 > 0 && peak.2 > 0);
    assert_eq!(manager.snapshot().unwrap(), Default::default());
}
