; The Windows installer of AudioRef, audioref-windows-x86_64-setup.exe, made with Inno Setup 6
; (https://jrsoftware.org/isinfo.php), see Windows installer in docs/DEVELOPMENT.md. release.yml
; compiles it from the files of the Windows archive:
;
;   ISCC /DVersion=0.1.0 /DBundles=<folder of the archive> /O<output folder> installer\audioref.iss
;
; Express installation installs the VST3 and the CLAP plugin and the user manual in English and
; German; custom installation lets one choose among them. The last page offers to open the manual.
;
; This file is UTF-8 with a byte order mark: without it, Inno Setup reads the German texts in the
; code page of the system.

#ifndef Version
  #error Pass AudioRef's version, e.g. /DVersion=0.1.0
#endif
#ifndef Bundles
  #error Pass the folder with AudioRef.vst3, AudioRef.clap and the manual, e.g. /DBundles=dist\audioref-windows-x86_64
#endif

; Windows wants up to four numbers in the version of a program: 1.0.0-rc.1 becomes 1.0.0.
#if Pos("-", Version) > 0
  #define NumericVersion Copy(Version, 1, Pos("-", Version) - 1)
#else
  #define NumericVersion Version
#endif

#define Repository "https://github.com/FionaPreroll/audioref"

[Setup]
; Windows finds an installed AudioRef by this ID, to update and to remove it: it never changes.
AppId={{20DCEC0A-386A-4ECD-B16A-10D6E07B520C}
AppName=AudioRef
AppVersion={#Version}
AppVerName=AudioRef {#Version}
AppPublisher=FionaPreroll
AppPublisherURL={#Repository}
AppSupportURL={#Repository}/issues
AppUpdatesURL={#Repository}/releases
AppCopyright=GPL-3.0-only
VersionInfoVersion={#NumericVersion}
VersionInfoProductTextVersion={#Version}
VersionInfoDescription=AudioRef Setup
; The manual, the licenses and the uninstaller. The plugins go to the folders where DAWs look for
; them, which don't change.
DefaultDirName={autopf}\AudioRef
DefaultGroupName=AudioRef
DisableDirPage=yes
DisableProgramGroupPage=yes
; The plugins go to C:\Program Files\Common Files, for all users.
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
; The language of Windows, English or German; the dialog only asks for others.
ShowLanguageDialog=auto
LanguageDetectionMethod=uilanguage
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputBaseFilename=audioref-windows-x86_64-setup
UninstallDisplayName=AudioRef

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[CustomMessages]
english.TypeExpress=Express installation: VST3, CLAP and the user manual
english.TypeCustom=Custom installation
english.ComponentVst3=VST3 plugin, in Common Files\VST3
english.ComponentClap=CLAP plugin, in Common Files\CLAP
english.ComponentManual=User manual (PDF, English and German)
english.ManualEnglish=AudioRef manual (English)
english.ManualGerman=AudioRef manual (German)
english.OpenManual=Open the user manual
english.OpenOnlineManual=Open the user manual on GitHub
english.NothingChosen=Choose at least one component.
german.TypeExpress=Express-Installation: VST3, CLAP und das Handbuch
german.TypeCustom=Benutzerdefinierte Installation
german.ComponentVst3=VST3-Plugin, in Common Files\VST3
german.ComponentClap=CLAP-Plugin, in Common Files\CLAP
german.ComponentManual=Handbuch (PDF, Deutsch und Englisch)
german.ManualEnglish=AudioRef-Handbuch (Englisch)
german.ManualGerman=AudioRef-Handbuch (Deutsch)
german.OpenManual=Handbuch öffnen
german.OpenOnlineManual=Handbuch auf GitHub öffnen
german.NothingChosen=Bitte mindestens eine Komponente auswählen.

[Types]
Name: "express"; Description: "{cm:TypeExpress}"
Name: "custom"; Description: "{cm:TypeCustom}"; Flags: iscustom

[Components]
Name: "vst3"; Description: "{cm:ComponentVst3}"; Types: express custom
Name: "clap"; Description: "{cm:ComponentClap}"; Types: express custom
Name: "manual"; Description: "{cm:ComponentManual}"; Types: express custom

[InstallDelete]
; The bundle of an earlier version, so no file of it stays behind in the new one.
Type: filesandordirs; Name: "{commoncf64}\VST3\AudioRef.vst3"; Components: vst3

[Files]
Source: "{#Bundles}\AudioRef.vst3\*"; DestDir: "{commoncf64}\VST3\AudioRef.vst3"; Components: vst3; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#Bundles}\AudioRef.clap"; DestDir: "{commoncf64}\CLAP"; Components: clap; Flags: ignoreversion
Source: "{#Bundles}\audioref-manual-en.pdf"; DestDir: "{app}"; Components: manual; Flags: ignoreversion
Source: "{#Bundles}\audioref-manual-de.pdf"; DestDir: "{app}"; Components: manual; Flags: ignoreversion
; AudioRef's license, and the source, the authors and the license of the built-in presets, as
; CC BY-SA 4.0 asks.
Source: "{#Bundles}\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#Bundles}\OPRA-PRESETS.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{cm:ManualEnglish}"; Filename: "{app}\audioref-manual-en.pdf"; Components: manual
Name: "{group}\{cm:ManualGerman}"; Filename: "{app}\audioref-manual-de.pdf"; Components: manual

[Run]
; The last page offers to open the manual in the language of the installer, or the one on GitHub
; if it wasn't installed.
Filename: "{app}\audioref-manual-en.pdf"; Description: "{cm:OpenManual}"; Components: manual; Languages: english; Flags: postinstall shellexec skipifsilent nowait
Filename: "{app}\audioref-manual-de.pdf"; Description: "{cm:OpenManual}"; Components: manual; Languages: german; Flags: postinstall shellexec skipifsilent nowait
Filename: "{#Repository}/blob/main/docs/manual/en.md"; Description: "{cm:OpenOnlineManual}"; Components: not manual; Languages: english; Flags: postinstall shellexec skipifsilent nowait unchecked
Filename: "{#Repository}/blob/main/docs/manual/de.md"; Description: "{cm:OpenOnlineManual}"; Components: not manual; Languages: german; Flags: postinstall shellexec skipifsilent nowait unchecked

[Code]
// Custom installation goes on only with at least one component.
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpSelectComponents) and (WizardSelectedComponents(False) = '') then
  begin
    MsgBox(CustomMessage('NothingChosen'), mbError, MB_OK);
    Result := False;
  end;
end;
