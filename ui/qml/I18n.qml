pragma Singleton
import QtQml
import waywallen.ui

QtObject {
    readonly property var revision: PluginTranslations.revision

    function tr(value) {
        const dependency = revision;
        return PluginTranslations.translate(value);
    }

    function optionLabel(options, value, fallback) {
        const raw = value === undefined || value === null ? "" : String(value);
        const option = (options ?? []).find(option => String(option.value) === raw);
        const label = tr(option?.labelText ?? option?.label ?? fallback ?? raw);
        return label.length > 0 ? label : raw;
    }
}
