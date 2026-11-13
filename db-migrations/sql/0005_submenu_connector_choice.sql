ALTER TABLE group_menu_opening_choices
    ADD COLUMN submenu_connector TEXT NOT NULL DEFAULT 'hidden'
        CHECK (submenu_connector IN ('hidden', 'shown'));
