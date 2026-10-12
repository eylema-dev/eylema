use eylema_platform::metal_add_i32;
use eylema_runtime::resource::{ResourceClass, ResourceLimits, ResourceManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ResourceManager::new(ResourceLimits::default())?;
    let metal_id = manager.admit(ResourceClass::Metal, 16 * 1024)?;
    manager.admit(ResourceClass::View, 512 * 1024)?;
    manager.admit(ResourceClass::Model, 1024 * 1024)?;

    let mut leases = Vec::new();
    for _ in 0..3 {
        leases.push(
            manager
                .dispatch_next()?
                .expect("three classes fit concurrently"),
        );
    }
    let metal = leases
        .iter()
        .find(|lease| lease.id() == metal_id)
        .expect("Metal request dispatched");
    metal.check_cancelled()?;

    let lhs: Vec<i32> = (0..256).collect();
    let rhs = vec![1_i32; lhs.len()];
    let result = metal_add_i32(&lhs, &rhs)?;
    assert_eq!(result.last(), Some(&256));
    println!(
        "M4 Metal lease passed; synthetic view/model reservations held concurrently; snapshot={:?}",
        manager.snapshot()?
    );
    drop(leases);
    assert_eq!(manager.snapshot()?.active, 0);
    assert_eq!(manager.snapshot()?.reserved_bytes, 0);
    Ok(())
}
