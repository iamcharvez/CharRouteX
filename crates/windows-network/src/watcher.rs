use network_core::NetworkError;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, HANDLE},
    NetworkManagement::IpHelper::{
        CancelMibChangeNotify2, MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
        NotifyIpInterfaceChange, NotifyRouteChange2,
    },
    Networking::WinSock::AF_UNSPEC,
};

struct CallbackContext {
    callback: Arc<dyn Fn() + Send + Sync>,
}

pub struct NetworkChangeWatcher {
    interface_handle: HANDLE,
    route_handle: HANDLE,
    interface_context: *const CallbackContext,
    route_context: *const CallbackContext,
}

// The OS owns callback execution after registration. Handles and retained Arc pointers remain
// valid until Drop synchronously cancels both registrations.
unsafe impl Send for NetworkChangeWatcher {}
unsafe impl Sync for NetworkChangeWatcher {}

impl NetworkChangeWatcher {
    pub fn start(callback: impl Fn() + Send + Sync + 'static) -> Result<Self, NetworkError> {
        let context = Arc::new(CallbackContext {
            callback: Arc::new(callback),
        });
        let interface_context = Arc::into_raw(context.clone());
        let mut interface_handle = HANDLE::default();
        let interface_result = unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(interface_callback),
                Some(interface_context.cast()),
                false,
                &mut interface_handle,
            )
        };
        if interface_result != ERROR_SUCCESS {
            unsafe { drop(Arc::from_raw(interface_context)) };
            return Err(NetworkError::Native(format!(
                "NotifyIpInterfaceChange returned Windows error {}",
                interface_result.0
            )));
        }

        let route_context = Arc::into_raw(context);
        let mut route_handle = HANDLE::default();
        let route_result = unsafe {
            NotifyRouteChange2(
                AF_UNSPEC,
                Some(route_callback),
                route_context.cast(),
                false,
                &mut route_handle,
            )
        };
        if route_result != ERROR_SUCCESS {
            unsafe {
                let _ = CancelMibChangeNotify2(interface_handle);
                drop(Arc::from_raw(interface_context));
                drop(Arc::from_raw(route_context));
            }
            return Err(NetworkError::Native(format!(
                "NotifyRouteChange2 returned Windows error {}",
                route_result.0
            )));
        }

        Ok(Self {
            interface_handle,
            route_handle,
            interface_context,
            route_context,
        })
    }
}

impl Drop for NetworkChangeWatcher {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelMibChangeNotify2(self.interface_handle);
            let _ = CancelMibChangeNotify2(self.route_handle);
            drop(Arc::from_raw(self.interface_context));
            drop(Arc::from_raw(self.route_context));
        }
    }
}

unsafe extern "system" fn interface_callback(
    caller_context: *const c_void,
    _row: *const MIB_IPINTERFACE_ROW,
    _notification_type: MIB_NOTIFICATION_TYPE,
) {
    invoke_callback(caller_context);
}

unsafe extern "system" fn route_callback(
    caller_context: *const c_void,
    _row: *const MIB_IPFORWARD_ROW2,
    _notification_type: MIB_NOTIFICATION_TYPE,
) {
    invoke_callback(caller_context);
}

fn invoke_callback(caller_context: *const c_void) {
    if caller_context.is_null() {
        return;
    }
    let context = unsafe { &*caller_context.cast::<CallbackContext>() };
    let _ = catch_unwind(AssertUnwindSafe(|| (context.callback)()));
}
