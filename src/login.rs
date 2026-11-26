//! Use macOS's registration state as the source of truth, not a saved preference.
use objc2_service_management::{SMAppService, SMAppServiceStatus};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Disabled,
    Enabled,
    ApprovalRequired,
    Unavailable,
}
pub fn status() -> Status {
    // Bundle minimum deployment target is macOS 13, where SMAppService is present.
    match unsafe { SMAppService::mainAppService().status() } {
        SMAppServiceStatus::NotRegistered => Status::Disabled,
        SMAppServiceStatus::Enabled => Status::Enabled,
        SMAppServiceStatus::RequiresApproval => Status::ApprovalRequired,
        _ => Status::Unavailable,
    }
}
pub fn set_enabled(enabled: bool) -> Result<Status, String> {
    let service = unsafe { SMAppService::mainAppService() };
    let result = unsafe {
        if enabled {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        }
    };
    result.map_err(|e| e.localizedDescription().to_string())?;
    Ok(status())
}
pub fn open_settings() {
    unsafe {
        SMAppService::openSystemSettingsLoginItems();
    }
}
