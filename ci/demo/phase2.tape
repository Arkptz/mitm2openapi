Output ci/demo/out/phase2.mp4
Output ci/demo/out/phase2.gif

Set Theme "Catppuccin Mocha"
Set FontFamily "JetBrains Mono"
Set FontSize 20
Set Width 1280
Set Height 720
Set Framerate 30
Set PlaybackSpeed 1
# Show the tool in action: convert crAPI flow to OpenAPI
Type "./target/release/mitm2openapi discover -i ci/demo/fixtures/demo.flow -o ci/demo/out/templates.yaml -p http://crapi-web:80"
Sleep 500ms
Enter
Sleep 2s
Sleep 3s

Type "# Edit templates.yaml to remove 'ignore:' prefix from desired endpoints"
Sleep 500ms
Enter
Sleep 500ms

Type "sed -i 's/^  ignore: /  /g' ci/demo/out/templates.yaml"
Sleep 500ms
Enter
Sleep 500ms

Type "./target/release/mitm2openapi generate -i ci/demo/fixtures/demo.flow -t ci/demo/out/templates.yaml -o ci/demo/out/openapi.yaml -p http://crapi-web:80"
Sleep 500ms
Enter
Sleep 3s
Sleep 3s

Type "head -30 ci/demo/out/openapi.yaml"
Sleep 500ms
Enter
Sleep 1s
Sleep 3s

Sleep 2s
