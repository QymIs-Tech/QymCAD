// Export forwarder for combase.dll on Windows 7.
// Windows 7 does not provide combase.dll (introduced in Windows 8).
// Standard COM functionality required by Rust windows-core is located in ole32.dll.
// The Windows NT PE loader automatically resolves forwarded exports to ole32.dll.

#pragma comment(linker, "/export:CoCreateFreeThreadedMarshaler=ole32.CoCreateFreeThreadedMarshaler")
#pragma comment(linker, "/export:CoCreateGuid=ole32.CoCreateGuid")
#pragma comment(linker, "/export:CoCreateInstance=ole32.CoCreateInstance")
#pragma comment(linker, "/export:CoCreateInstanceEx=ole32.CoCreateInstanceEx")
#pragma comment(linker, "/export:CoGetApartmentType=ole32.CoGetApartmentType")
#pragma comment(linker, "/export:CoGetClassObject=ole32.CoGetClassObject")
#pragma comment(linker, "/export:CoGetMalloc=ole32.CoGetMalloc")
#pragma comment(linker, "/export:CoGetObjectContext=ole32.CoGetObjectContext")
#pragma comment(linker, "/export:CoInitializeEx=ole32.CoInitializeEx")
#pragma comment(linker, "/export:CoRegisterClassObject=ole32.CoRegisterClassObject")
#pragma comment(linker, "/export:CoRevokeClassObject=ole32.CoRevokeClassObject")
#pragma comment(linker, "/export:CoTaskMemAlloc=ole32.CoTaskMemAlloc")
#pragma comment(linker, "/export:CoTaskMemFree=ole32.CoTaskMemFree")
#pragma comment(linker, "/export:CoTaskMemRealloc=ole32.CoTaskMemRealloc")
#pragma comment(linker, "/export:CoUninitialize=ole32.CoUninitialize")
#pragma comment(linker, "/export:CoWaitForMultipleHandles=ole32.CoWaitForMultipleHandles")
#pragma comment(linker, "/export:CLSIDFromString=ole32.CLSIDFromString")
#pragma comment(linker, "/export:IIDFromString=ole32.IIDFromString")
#pragma comment(linker, "/export:PropVariantClear=ole32.PropVariantClear")
#pragma comment(linker, "/export:PropVariantCopy=ole32.PropVariantCopy")
#pragma comment(linker, "/export:StringFromCLSID=ole32.StringFromCLSID")
#pragma comment(linker, "/export:StringFromGUID2=ole32.StringFromGUID2")

int DllMainCRTStartup(void) {
    return 1;
}
