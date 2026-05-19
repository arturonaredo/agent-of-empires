import { useEffect, useState } from "react";
import { fetchThemes } from "../../lib/api";
import { dispatchThemePickerChanged } from "../../hooks/useResolvedTheme";
import { SelectField } from "./FormFields";

interface Props {
  settings: Record<string, unknown>;
  onSaveField: (section: string, field: string, value: unknown) => void;
  onUpdate: (patch: Record<string, unknown>) => void;
}

export function ThemeSettings({ settings, onSaveField, onUpdate }: Props) {
  const [themes, setThemes] = useState<string[]>([]);
  const theme = (settings.theme ?? {}) as Record<string, unknown>;

  useEffect(() => {
    fetchThemes().then(setThemes);
  }, []);

  const save = (field: string, value: unknown) => {
    onUpdate({ theme: { ...theme, [field]: value } });
    onSaveField("theme", field, value);
    // Repaint the web dashboard immediately on theme pick. The picker
    // writes config.toml synchronously above; tell useResolvedTheme to
    // refetch and apply without waiting for a full settings sync.
    if (field === "name") {
      dispatchThemePickerChanged(typeof value === "string" ? value : undefined);
    }
  };

  return (
    <div className="space-y-4">
      <p className="text-xs text-text-dim">
        Theme applies to both the TUI and the web dashboard. Custom themes
        in <code>~/.ai-cli-manager/themes/*.toml</code> appear alongside
        the builtins.
      </p>
      <SelectField
        label="Theme"
        value={(theme.name as string) ?? ""}
        onChange={(v) => save("name", v)}
        options={themes.map((t) => ({ value: t, label: t }))}
      />

    </div>
  );
}
