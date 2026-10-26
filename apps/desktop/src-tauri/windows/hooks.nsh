; Hooks of the Windows installer (V0.4.50). The installer is per person (installMode currentUser: no administrator rights, nothing
; outside the person's own folders), installs silently with /S, puts a Start menu entry and a desktop shortcut (both are the
; template's own; /NS leaves them out) and its uninstaller offers to forget what the app keeps for the person.

; Before the files go: ask whether to forget the saved server and the saved sign-in (the session in the Windows credential store).
; A silent removal (/S) asks nothing and keeps them, so an update that removes and installs again keeps the person signed in; to
; forget on a silent removal, run "RepoMemo.exe --forget-session" first.
!macro NSIS_HOOK_PREUNINSTALL
  IfSilent keep_session
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Also forget the server and the sign-in RepoMemo saved on this computer?$\r$\n$\r$\nYour account and everything in it stay on the server. Choose No to keep them for a later install." IDNO keep_session
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --forget-session'
  keep_session:
!macroend
