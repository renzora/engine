use crate::nvsdk_ngx::*;
use std::{
    env::{self, var},
    ffi::{CString, OsStr, OsString},
    ptr,
};
use uuid::Uuid;

pub fn with_feature_info<F, T>(project_id: Uuid, feature_id: NVSDK_NGX_Feature, callback: F) -> T
where
    F: FnOnce(&NVSDK_NGX_FeatureDiscoveryInfo) -> T,
{
    let project_id = CString::new(project_id.to_string()).unwrap();
    let engine_version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    let data_path = os_str_to_wchar(env::temp_dir().as_os_str());

    let shared_library_paths = get_shared_library_paths();
    let shared_library_path_pointers = shared_library_paths
        .iter()
        .map(Vec::as_ptr)
        .collect::<Vec<_>>();

    let feature_info_common = NVSDK_NGX_FeatureCommonInfo {
        PathListInfo: NVSDK_NGX_PathListInfo {
            Path: shared_library_path_pointers.as_ptr(),
            Length: shared_library_paths.len() as u32,
        },
        InternalData: ptr::null_mut(),
        // TODO: Allow configuring logging
        LoggingInfo: NVSDK_NGX_LoggingInfo {
            LoggingCallback: None,
            MinimumLoggingLevel: NVSDK_NGX_Logging_Level_NVSDK_NGX_LOGGING_LEVEL_OFF,
            DisableOtherLoggingSinks: false,
        },
    };

    let feature_info = NVSDK_NGX_FeatureDiscoveryInfo {
        SDKVersion: NVSDK_NGX_Version_NVSDK_NGX_Version_API,
        FeatureID: feature_id,
        Identifier: NVSDK_NGX_Application_Identifier {
            IdentifierType: NVSDK_NGX_Application_Identifier_Type_NVSDK_NGX_Application_Identifier_Type_Project_Id,
            v: NVSDK_NGX_Application_Identifier_v {
                ProjectDesc: NVSDK_NGX_ProjectIdDescription {
                    ProjectId: project_id.as_ptr(),
                    EngineType: NVSDK_NGX_EngineType_NVSDK_NGX_ENGINE_TYPE_CUSTOM,
                    EngineVersion: engine_version.as_ptr(),
                },
            },
        },
        ApplicationDataPath: data_path.as_ptr(),
        FeatureInfo: &feature_info_common,
    };

    (callback)(&feature_info)
}

fn get_shared_library_paths() -> Vec<Vec<wchar_t>> {
    let mut shared_library_paths = vec![];

    // Set by build.rs
    let platform = env!("DLSS_SDK_PLATFORM");

    #[cfg(feature = "debug_overlay")]
    let profile = "dev";
    #[cfg(not(feature = "debug_overlay"))]
    let profile = "rel";

    // Look in $DLSS_SDK if set
    let sdk_path = var("DLSS_SDK").map(|sdk| format!("{sdk}/lib/{platform}/{profile}"));
    if let Ok(sdk_path) = sdk_path.as_ref() {
        shared_library_paths.push(os_str_to_wchar(&OsString::from(sdk_path)));
    }

    shared_library_paths
}

#[cfg(target_os = "windows")]
fn os_str_to_wchar(s: &OsStr) -> Vec<wchar_t> {
    use std::os::windows::ffi::OsStrExt;

    s.encode_wide().chain([0]).map(|c| c as wchar_t).collect()
}

#[cfg(not(target_os = "windows"))]
fn os_str_to_wchar(s: &OsStr) -> Vec<wchar_t> {
    s.to_str()
        .unwrap_or("")
        .chars()
        .chain([0 as char])
        .map(|c| c as wchar_t)
        .collect()
}
