use std::ffi::{c_void, CStr};

use ash::vk;
use log::{debug, error, trace, warn};

pub extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    let _data = unsafe { *p_callback_data };
    let message = unsafe { CStr::from_ptr((*p_callback_data).p_message) };

    if message_severity >= vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
        error!("[VALIDATION] ({:?}) {:?}", message_type, message);
    } else if message_severity >= vk::DebugUtilsMessageSeverityFlagsEXT::WARNING {
        warn!("[VALIDATION] ({:?}) {:?}", message_type, message);
    } else if message_severity >= vk::DebugUtilsMessageSeverityFlagsEXT::INFO {
        debug!("[VALIDATION] ({:?}) {:?}", message_type, message);
    } else {
        trace!("[VALIDATION] ({:?}) {:?}", message_type, message);
    }

    vk::FALSE
}
