//! Task/Need-bound real-person control; no raw host or proof constructor.
use super::*;
use crate::human::HumanEnrollment;
use eylema_platform::human::{DeviceOwnerProof, HumanError, HumanPurpose};
use std::time::Duration;
pub struct ReportControlRequest {
    enrollment: HumanEnrollment,
    task: TaskId,
    need: [u8; 32],
}
pub struct ReportController {
    pub(crate) human: DeviceOwnerHuman,
    pub(crate) task: TaskId,
    pub(crate) need: [u8; 32],
}
impl std::fmt::Debug for ReportControlRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReportControlRequest(<opaque>)")
    }
}
impl std::fmt::Debug for ReportController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReportController(<opaque>)")
    }
}
impl ReportControlRequest {
    pub fn authenticate(
        &self,
        timeout: Duration,
    ) -> std::result::Result<DeviceOwnerProof, HumanError> {
        self.enrollment.authenticate(timeout)
    }
}
impl ReportController {
    pub fn remaining(&self) -> Duration {
        self.human.remaining()
    }
    pub fn deadline(&self) -> Timestamp {
        self.human.deadline()
    }
}
impl<T: Storage, H: Sha256, S: HostServices> ToolHost<T, H, S> {
    pub fn request_report_controller(
        &self,
        owner: &Owner,
        grant: Grant,
        task: TaskId,
        need: [u8; 32],
        ttl: Duration,
    ) -> std::result::Result<ReportControlRequest, ToolError> {
        let enrollment = self.host.request_device_owner_bound(
            owner,
            grant,
            HumanPurpose::LocalReport,
            ttl,
            Some((&task, need)),
        )?;
        Ok(ReportControlRequest {
            enrollment,
            task,
            need,
        })
    }
    pub fn enroll_report_controller(
        &mut self,
        owner: &Owner,
        request: ReportControlRequest,
        proof: DeviceOwnerProof,
    ) -> std::result::Result<ReportController, ToolError> {
        let human = self
            .host
            .enroll_device_owner_bound(owner, request.enrollment, proof)?;
        Ok(ReportController {
            human,
            task: request.task,
            need: request.need,
        })
    }
    pub fn check_report_controller(
        &self,
        controller: &ReportController,
        task: &TaskId,
        need: [u8; 32],
    ) -> std::result::Result<(), ToolError> {
        if &controller.task != task
            || controller.need != need
            || controller.human.remaining().is_zero()
        {
            return Err(ToolError::Runtime(ErrorCode::Forbidden));
        }
        self.host.session(controller.human.session())?;
        Ok(())
    }
    pub fn approve_report_checkpoint(
        &mut self,
        controller: &ReportController,
        need: [u8; 32],
        plan: &ToolPlan,
        binding: ApprovalBinding,
    ) -> std::result::Result<EventId, ToolError> {
        self.check_report_controller(controller, &plan.task().fields().task_id, need)?;
        self.registry.validate(plan).map_err(ToolError::Runtime)?;
        Ok(self.host.device_owner_approval(
            &controller.human,
            plan.task(),
            plan.proposal(),
            binding,
        )?)
    }
    pub fn cancel_report_checkpoint(
        &mut self,
        controller: &ReportController,
        task: &TaskId,
        need: [u8; 32],
        id: &ExecutionId,
    ) -> std::result::Result<CancellationOutcome, ToolError> {
        self.check_report_controller(controller, task, need)?;
        if self
            .records
            .get(id)
            .is_none_or(|r| &r.plan.task().fields().task_id != task)
        {
            return Err(ToolError::InvalidEvidence);
        }
        let started = Instant::now();
        let result = self.host.device_owner_cancel(&controller.human, id)?;
        if matches!(&result,CancellationOutcome::Requested(r) if &r.execution_id==id)
            && let Some(p) = &mut self.active
            && &p.id == id
        {
            p.cancel_requested = true;
            self.records
                .get_mut(id)
                .ok_or(ToolError::InvalidEvidence)?
                .measurement
                .cancellation_started
                .get_or_insert(started);
        }
        Ok(result)
    }
    pub fn revoke_report_controller(
        &mut self,
        controller: &ReportController,
    ) -> std::result::Result<(), ToolError> {
        Ok(self.host.revoke_device_owner(&controller.human)?)
    }
}

#[cfg(test)]
impl<T: Storage, H: Sha256, S: HostServices> ToolHost<T, H, S> {
    pub(crate) fn simulate_report_controller(
        &mut self,
        owner: &Owner,
        grant: Grant,
        task: &TaskId,
        need: [u8; 32],
    ) -> ReportController {
        let human = crate::human::simulate_report_person(&mut self.host, owner, grant, task, need);
        ReportController {
            human,
            task: task.clone(),
            need,
        }
    }
}
impl<T: Storage, H: Sha256, S: HostServices> ToolHost<T, H, S> {
    pub fn create_report_task(
        &mut self,
        owner: &Owner,
        actor: &DurableSession,
        controller: &ReportController,
        need: [u8; 32],
        task: TaskSpec,
    ) -> std::result::Result<(), ToolError> {
        self.check_report_controller(controller, &task.fields().task_id, need)?;
        let scope = &task.fields().resource_scope;
        if scope.resources().len() != 1
            || scope
                .resources()
                .next()
                .is_none_or(|r| r.as_str() != HumanPurpose::LocalReport.resource())
        {
            return Err(ToolError::Runtime(ErrorCode::Forbidden));
        }
        let oversight = OversightRequirements::new(vec![], true, true, None)
            .map_err(|_| ToolError::Runtime(ErrorCode::InvalidRequest))?;
        Ok(self
            .host
            .create_governed_task(owner, actor, task, oversight)?)
    }
}
