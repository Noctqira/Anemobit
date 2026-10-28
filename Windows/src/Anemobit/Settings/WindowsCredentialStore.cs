using System.Runtime.InteropServices;
using System.Text;

namespace Anemobit.Settings;

/// <summary>
/// Windows Credential Manager (generic credentials). Target names are <c>Anemobit:snmp/{name}</c>, stored in the current user's profile.
/// Contents are stored as UTF-16 (matching how Credential Manager shows passwords).
/// </summary>
public sealed partial class WindowsCredentialStore(string targetPrefix = "Anemobit:snmp/") : ICredentialStore
{
    private const int CredTypeGeneric = 1;
    private const int CredPersistLocalMachine = 2;
    private const int ErrorNotFound = 1168;

    public string? Read(string name)
    {
        if (!CredRead(Target(name), CredTypeGeneric, 0, out var handle))
        {
            return null;
        }
        try
        {
            var credential = Marshal.PtrToStructure<NativeCredential>(handle);
            return credential.CredentialBlobSize == 0
                ? string.Empty
                : Marshal.PtrToStringUni(credential.CredentialBlob, (int)credential.CredentialBlobSize / 2);
        }
        finally
        {
            CredFree(handle);
        }
    }

    public bool Write(string name, string secret)
    {
        ArgumentNullException.ThrowIfNull(secret);
        var blob = Encoding.Unicode.GetBytes(secret);
        var blobHandle = Marshal.AllocHGlobal(Math.Max(blob.Length, 1));
        var targetHandle = Marshal.StringToHGlobalUni(Target(name));
        var userHandle = Marshal.StringToHGlobalUni(Environment.UserName);
        try
        {
            Marshal.Copy(blob, 0, blobHandle, blob.Length);
            var credential = new NativeCredential
            {
                Type = CredTypeGeneric,
                TargetName = targetHandle,
                CredentialBlobSize = (uint)blob.Length,
                CredentialBlob = blobHandle,
                Persist = CredPersistLocalMachine,
                UserName = userHandle,
            };
            return CredWrite(ref credential, 0);
        }
        finally
        {
            // Clear the temporary password contents before freeing.
            Marshal.Copy(new byte[blob.Length], 0, blobHandle, blob.Length);
            Array.Clear(blob);
            Marshal.FreeHGlobal(blobHandle);
            Marshal.FreeHGlobal(targetHandle);
            Marshal.FreeHGlobal(userHandle);
        }
    }

    public bool Delete(string name) =>
        CredDelete(Target(name), CredTypeGeneric, 0) || Marshal.GetLastPInvokeError() == ErrorNotFound;

    private string Target(string name) => targetPrefix + name;

    [LibraryImport("advapi32.dll", EntryPoint = "CredReadW", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredRead(string target, int type, int flags, out IntPtr credential);

    [LibraryImport("advapi32.dll", EntryPoint = "CredWriteW", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredWrite(ref NativeCredential credential, int flags);

    [LibraryImport("advapi32.dll", EntryPoint = "CredDeleteW", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CredDelete(string target, int type, int flags);

    [LibraryImport("advapi32.dll")]
    private static partial void CredFree(IntPtr buffer);

    /// <summary>CREDENTIALW.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct NativeCredential
    {
        public int Flags;
        public int Type;
        public IntPtr TargetName;
        public IntPtr Comment;
        public long LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public int Persist;
        public int AttributeCount;
        public IntPtr Attributes;
        public IntPtr TargetAlias;
        public IntPtr UserName;
    }
}
