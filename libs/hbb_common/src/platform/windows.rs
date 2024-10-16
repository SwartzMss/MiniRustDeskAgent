use winapi::{
    shared::minwindef::{DWORD, TRUE},
    um::{
        sysinfoapi::VerSetConditionMask,
        winbase::VerifyVersionInfoW,
        winnt::{
            OSVERSIONINFOEXW, VER_BUILDNUMBER, VER_GREATER_EQUAL, VER_MAJORVERSION,
            VER_MINORVERSION, VER_SERVICEPACKMAJOR, VER_SERVICEPACKMINOR,
        },
    },
};

// https://learn.microsoft.com/en-us/windows/win32/sysinfo/targeting-your-application-at-windows-8-1
// https://github.com/nodejs/node-convergence-archive/blob/e11fe0c2777561827cdb7207d46b0917ef3c42a7/deps/uv/src/win/util.c#L780
pub fn is_windows_version_or_greater(
    os_major: u32,
    os_minor: u32,
    build_number: u32,
    service_pack_major: u32,
    service_pack_minor: u32,
) -> bool {
    let mut osvi: OSVERSIONINFOEXW = unsafe { std::mem::zeroed() };
    osvi.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXW>() as DWORD;
    osvi.dwMajorVersion = os_major as _;
    osvi.dwMinorVersion = os_minor as _;
    osvi.dwBuildNumber = build_number as _;
    osvi.wServicePackMajor = service_pack_major as _;
    osvi.wServicePackMinor = service_pack_minor as _;

    let result = unsafe {
        let mut condition_mask = 0;
        let op = VER_GREATER_EQUAL;
        condition_mask = VerSetConditionMask(condition_mask, VER_MAJORVERSION, op);
        condition_mask = VerSetConditionMask(condition_mask, VER_MINORVERSION, op);
        condition_mask = VerSetConditionMask(condition_mask, VER_BUILDNUMBER, op);
        condition_mask = VerSetConditionMask(condition_mask, VER_SERVICEPACKMAJOR, op);
        condition_mask = VerSetConditionMask(condition_mask, VER_SERVICEPACKMINOR, op);

        VerifyVersionInfoW(
            &mut osvi as *mut OSVERSIONINFOEXW,
            VER_MAJORVERSION
                | VER_MINORVERSION
                | VER_BUILDNUMBER
                | VER_SERVICEPACKMAJOR
                | VER_SERVICEPACKMINOR,
            condition_mask,
        )
    };

    result == TRUE
}
