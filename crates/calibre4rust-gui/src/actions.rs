use gpui::actions;

actions!(
    calibre4rust,
    [
        AddBooks,
        EditMetadata,
        ConvertBooks,
        ChooseLibrary,
        SaveToDisk,
        OpenPreferences,
        ShowHelp,
        RemoveBooks,
        TweakEpub,
        ToggleLayout,
        ToggleCoverGrid,
        ToggleBookDetails,
        ToggleTagBrowser,
    ]
);
