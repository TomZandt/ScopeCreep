; ScopeCreep's Windows Installer (#251, spec #235), compiled with Inno Setup 6.3 or later by
; scripts/package-windows.sh, which stages this script beside scopecreep.exe, scopecreep_sender.ofx.bundle,
; INSTALL.txt and the version.iss it writes. Unsigned (code signing is out of scope): INSTALL.txt explains
; SmartScreen's "Windows protected your PC" warning.

#include "version.iss"
#ifndef AppVersion
  #error version.iss must define AppVersion and OutputName (scripts/package-windows.sh writes it)
#endif

[Setup]
; Never change AppId: it is how a newer Installer finds and replaces the installed one.
AppId={{71604776-1069-4F58-AA04-F43DD41A7620}
AppName=ScopeCreep
AppVersion={#AppVersion}
AppVerName=ScopeCreep {#AppVersion}
AppPublisher=Tom Zandt-Valentine
AppPublisherURL=https://github.com/TomZandt/ScopeCreep
AppSupportURL=https://github.com/TomZandt/ScopeCreep/issues
; Per machine: the Sender goes in a folder every user's Resolve reads, which needs admin rights.
PrivilegesRequired=admin
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0
DefaultDirName={autopf}\ScopeCreep
DisableProgramGroupPage=yes
Uninstallable=yes
UninstallDisplayName=ScopeCreep
UninstallDisplayIcon={app}\scopecreep.exe
; A running ScopeCreep (an Update started from the app) is closed through the Restart Manager.
CloseApplications=yes
DisableWelcomePage=no
InfoBeforeFile=INSTALL.txt
OutputDir=Output
OutputBaseFilename={#OutputName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Messages]
WelcomeLabel2=This will install [name/ver] on your computer.%n%nScopeCreep needs an NVIDIA GPU and DaVinci Resolve Studio with GPU processing set to CUDA (Preferences > System > Memory and GPU). AMD and Intel GPUs are not supported.%n%nClose DaVinci Resolve before you continue.
FinishedLabel=Setup has installed [name] on your computer.%n%nStart DaVinci Resolve, add ScopeCreep from Effects > OpenFX to a node on the Color page, then start ScopeCreep from the Start menu.

[InstallDelete]
; An older Sender's files never linger in the new bundle.
Type: filesandordirs; Name: "{commoncf64}\OFX\Plugins\scopecreep_sender.ofx.bundle"

[Files]
Source: "scopecreep.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "INSTALL.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "scopecreep_sender.ofx.bundle\*"; DestDir: "{commoncf64}\OFX\Plugins\scopecreep_sender.ofx.bundle"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\ScopeCreep"; Filename: "{app}\scopecreep.exe"

[UninstallDelete]
Type: filesandordirs; Name: "{commoncf64}\OFX\Plugins\scopecreep_sender.ofx.bundle"

[Code]
const
  ResolveExe = 'Resolve.exe';

{ Whether a process with this file name runs, asked of WMI. If Windows cannot say, carry on: the Restart
  Manager still catches a Sender file Resolve holds open. }
function ProcessRunning(const ExeName: String): Boolean;
var
  Locator, Service, Found: Variant;
begin
  Result := False;
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Service := Locator.ConnectServer('.', 'root\CIMV2');
    Found := Service.ExecQuery('SELECT ProcessId FROM Win32_Process WHERE Name = ''' + ExeName + '''');
    Result := Found.Count > 0;
  except
    Log('Could not ask Windows which programs are running: ' + GetExceptionMessage);
  end;
end;

{ True once Resolve is closed; False when the person cancels (or, in a silent run, at once). }
function ResolveClosed(const Action: String): Boolean;
begin
  Result := True;
  while ProcessRunning(ResolveExe) do
  begin
    if SuppressibleMsgBox('Close DaVinci Resolve to ' + Action + ' ScopeCreep.' + #13#10#13#10 +
         'Resolve keeps the Sender open while it runs. Close Resolve, then click Retry.',
         mbError, MB_RETRYCANCEL, IDCANCEL) = IDCANCEL then
    begin
      Result := False;
      Exit;
    end;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := ResolveClosed('install');
end;

function InitializeUninstall(): Boolean;
begin
  Result := ResolveClosed('uninstall');
end;
