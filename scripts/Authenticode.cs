using System;
using System.Runtime.InteropServices;

namespace SnipSigning
{
    public static class Authenticode
    {
        public const int UntrustedRoot = unchecked((int)0x800B0109);

        public static int Verify(string path)
        {
            var file = new FileInfo { Size = (uint)Marshal.SizeOf(typeof(FileInfo)), Path = path };
            var filePointer = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(FileInfo)));
            Marshal.StructureToPtr(file, filePointer, false);
            var action = new Guid("00AAC56B-CD44-11D0-8CC2-00C04FC295EE"); // WINTRUST_ACTION_GENERIC_VERIFY_V2
            var data = new TrustData
            {
                Size = (uint)Marshal.SizeOf(typeof(TrustData)),
                UIChoice = 2, // WTD_UI_NONE
                UnionChoice = 1, // WTD_CHOICE_FILE
                File = filePointer,
                StateAction = 1, // WTD_STATEACTION_VERIFY
            };
            try
            {
                return WinVerifyTrust(new IntPtr(-1), ref action, ref data);
            }
            finally
            {
                data.StateAction = 2; // WTD_STATEACTION_CLOSE
                WinVerifyTrust(new IntPtr(-1), ref action, ref data);
                Marshal.DestroyStructure(filePointer, typeof(FileInfo));
                Marshal.FreeHGlobal(filePointer);
            }
        }

        [DllImport("wintrust.dll", ExactSpelling = true)]
        private static extern int WinVerifyTrust(IntPtr window, ref Guid action, ref TrustData data);

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct FileInfo
        {
            public uint Size;
            [MarshalAs(UnmanagedType.LPWStr)] public string Path;
            public IntPtr Handle;
            public IntPtr KnownSubject;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct TrustData
        {
            public uint Size;
            public IntPtr PolicyCallbackData;
            public IntPtr SIPClientData;
            public uint UIChoice;
            public uint RevocationChecks;
            public uint UnionChoice;
            public IntPtr File;
            public uint StateAction;
            public IntPtr StateData;
            public IntPtr URLReference;
            public uint ProviderFlags;
            public uint UIContext;
            public IntPtr SignatureSettings;
        }
    }
}
