#[macro_export]
macro_rules! simengine_plugin {
    ($sim_ty:ty) => {
        extern "C" fn __simengine_create(
            ctx: $crate::SimContext,
            config_json: *const ::std::ffi::c_char,
        ) -> *mut ::std::ffi::c_void {
            let config_json = unsafe { ::std::ffi::CStr::from_ptr(config_json) }
                .to_string_lossy()
                .into_owned();

            let ctx = $crate::SimulationContext::from_raw(ctx);
            let sim = <$sim_ty as $crate::Simulation>::create(ctx, &config_json);
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(sim)) as *mut ::std::ffi::c_void
        }

        extern "C" fn __simengine_pre_step(instance: *mut ::std::ffi::c_void, dt_seconds: f64) {
            let sim = unsafe { &mut *(instance as *mut $sim_ty) };
            <$sim_ty as $crate::Simulation>::pre_step(sim, dt_seconds);
        }

        extern "C" fn __simengine_step(instance: *mut ::std::ffi::c_void, dt_seconds: f64) {
            let sim = unsafe { &mut *(instance as *mut $sim_ty) };
            <$sim_ty as $crate::Simulation>::step(sim, dt_seconds);
        }

        extern "C" fn __simengine_post_step(instance: *mut ::std::ffi::c_void, dt_seconds: f64) {
            let sim = unsafe { &mut *(instance as *mut $sim_ty) };
            <$sim_ty as $crate::Simulation>::post_step(sim, dt_seconds);
        }

        extern "C" fn __simengine_destroy(instance: *mut ::std::ffi::c_void) {
            if !instance.is_null() {
                let sim = unsafe { &mut *(instance as *mut $sim_ty) };
                <$sim_ty as $crate::Simulation>::destroy(sim);
                unsafe {
                    drop(::std::boxed::Box::from_raw(instance as *mut $sim_ty));
                }
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn simengine_get_api() -> $crate::SimApi {
            $crate::SimApi {
                api_version: $crate::SIMENGINE_API_VERSION,
                create: __simengine_create,
                pre_step: __simengine_pre_step,
                step: __simengine_step,
                post_step: __simengine_post_step,
                destroy: __simengine_destroy,
            }
        }
    };
}
