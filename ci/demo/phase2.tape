Output out/phase2.mp4
Output out/phase2.gif

Set Theme "Catppuccin Mocha"
Set FontFamily "JetBrains Mono"
Set FontSize 20
Set Width 1280
Set Height 720
Set Framerate 30
Set PlaybackSpeed 1

# Show the tool in action: convert flow to OpenAPI
Type "mitm2openapi discover -i ci/demo/fixtures/demo.flow -o out/templates.yaml -p http://demo-app:3000"
Sleep 500ms
Enter
Sleep 2s
Wait+Screen "templates.yaml"

Type "# Edit templates.yaml to remove 'ignore:' prefix from desired endpoints"
Sleep 500ms
Enter
Sleep 500ms

Type "sed -i 's/^  ignore: /  /g' out/templates.yaml"
Sleep 500ms
Enter
Sleep 500ms

Type "mitm2openapi generate -i ci/demo/fixtures/demo.flow -t out/templates.yaml -o out/openapi.yaml -p http://demo-app:3000"
Sleep 500ms
Enter
Sleep 3s
Wait+Screen "openapi.yaml"

Type "head -30 out/openapi.yaml"
Sleep 500ms
Enter
Sleep 1s
Wait+Screen "openapi:"

Sleep 2s
