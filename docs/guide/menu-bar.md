# Menu bar

Top panel, left. The focused window's menus, with nested submenus and separators as the application exported them. Both settings are in the Menu bar group of the [Settings](settings.md) menu and are on by default.

| Setting | On | Off |
|---|---|---|
| App menus in panel | the focused window's menus show in the panel | nothing shows |
| One panel button per menu | each top-level menu (File, Edit, …) is its own panel button | all top-level menus sit behind one Menu bar button |

While the menus are on, the shell registers as the global menu registrar, so Qt apps started afterwards hide their own menu bar.
