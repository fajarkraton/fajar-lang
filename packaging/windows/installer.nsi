; Fajar Lang NSIS Installer
; Build: makensis installer.nsi

!define PRODUCT_NAME "Fajar Lang"
!define PRODUCT_VERSION "6.1.0"
!define PRODUCT_PUBLISHER "PrimeCore.id"
!define PRODUCT_WEB_SITE "https://github.com/fajarkraton/fajar-lang"
!define PRODUCT_EXE "fj.exe"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "fj-${PRODUCT_VERSION}-setup.exe"
InstallDir "$PROGRAMFILES64\FajarLang"
RequestExecutionLevel admin

Section "Install"
    SetOutPath $INSTDIR
    File "..\..\target\release\fj.exe"

    ; Add to PATH
    EnVar::AddValue "PATH" "$INSTDIR"

    ; Start menu shortcut
    CreateDirectory "$SMPROGRAMS\Fajar Lang"
    CreateShortCut "$SMPROGRAMS\Fajar Lang\Fajar Lang REPL.lnk" "$INSTDIR\fj.exe" "repl"

    ; Uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Registry for Add/Remove Programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\FajarLang" \
        "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\FajarLang" \
        "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\FajarLang" \
        "DisplayVersion" "${PRODUCT_VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\FajarLang" \
        "Publisher" "${PRODUCT_PUBLISHER}"
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\fj.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    Delete "$SMPROGRAMS\Fajar Lang\Fajar Lang REPL.lnk"
    RMDir "$SMPROGRAMS\Fajar Lang"
    EnVar::DeleteValue "PATH" "$INSTDIR"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\FajarLang"
SectionEnd
