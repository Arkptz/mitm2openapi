Output ci/demo/out/phase2.mp4
Output ci/demo/out/phase2.gif

Set Theme "Catppuccin Mocha"
Set FontFamily "JetBrains Mono"
Set FontSize 20
Set Width 1280
Set Height 720
Set Framerate 30
Set PlaybackSpeed 1

Type "./target/release/mitm2openapi discover -i ci/demo/out/demo.flow -o ci/demo/out/templates.yaml -p http://crapi-web --exclude-patterns '/static/**,/images/**,*.css,*.js,*.svg' --include-patterns '/identity/**,/workshop/**,/community/**'"
Sleep 500ms
Enter
Sleep 5s

Type "./target/release/mitm2openapi generate -i ci/demo/out/demo.flow -t ci/demo/out/templates.yaml -o ci/demo/out/openapi.yaml -p http://crapi-web"
Sleep 500ms
Enter
Sleep 8s

Type "head -30 ci/demo/out/openapi.yaml"
Sleep 500ms
Enter
Sleep 1s
Sleep 3s

Sleep 2s
