import { open } from "@tauri-apps/plugin-dialog";

export interface DirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

export async function chooseDirectory(
  options: DirectoryPickerOptions,
): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    ...options,
  });
  return typeof selected === "string" ? selected : null;
}
