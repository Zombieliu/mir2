# Logout returns to character selection

Crystal GameScene.LogOutSuccess constructs SelectScene(p.Characters). Native
shell incorrectly switched to Login both on sending Logout and receiving its
success response. It now waits in the current scene for the server response,
then uses the returned character roster in CharacterSelect. An existing valid
selection is retained, otherwise the first roster entry is selected. A failed
operation releases the pending logout guard and retains the active character.
Passwords still clear; connection/authentication are not bypassed or recreated.

The door button confirmation now says Return to character selection; Exit
remains a separate action and uses the current Numeron product name.

Full native-ui library tests: 877/877. Regressions exercise success roster,
duplicate request rejection, failed logout/retry and subsequent StartGame.
Actual live logout/character-switch acceptance remains pending new-package
handoff; no old screenshots are treated as a pass.
