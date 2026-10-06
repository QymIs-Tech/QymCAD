// Export forwarder for combase.dll on Windows 7.
// Windows 7 does not provide combase.dll (introduced in Windows 8).
// Basic COM functionality required by Rust windows-core is located in ole32.dll.
// The Windows NT PE loader automatically resolves forwarded exports to ole32.dll.

int DllMainCRTStartup(void) {
    return 1;
}
