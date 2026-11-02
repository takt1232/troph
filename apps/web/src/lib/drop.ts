export interface DroppedFile {
  file: File;
  name: string;
}

type DataTransferItemWithEntries = DataTransferItem & {
  getAsEntry?: () => FileSystemEntry | null;
  webkitGetAsEntry?: () => FileSystemEntry | null;
};

type FileWithRelativePath = File & { webkitRelativePath?: string };

function entryForItem(item: DataTransferItem): FileSystemEntry | null {
  const candidate = item as DataTransferItemWithEntries;
  try {
    if (typeof candidate.getAsEntry === 'function') return candidate.getAsEntry();
    if (typeof candidate.webkitGetAsEntry === 'function') return candidate.webkitGetAsEntry();
  } catch {
    return null;
  }
  return null;
}

function normalizeEntryPath(path: string): string {
  return path.replace(/^\/+/, '');
}

function readFileEntry(entry: FileSystemFileEntry): Promise<File> {
  return new Promise((resolve, reject) => entry.file(resolve, reject));
}

function readEntryBatch(reader: FileSystemDirectoryReader): Promise<FileSystemEntry[]> {
  return new Promise((resolve, reject) => reader.readEntries(resolve, reject));
}

async function readDirectoryEntries(
  entry: FileSystemDirectoryEntry,
): Promise<FileSystemEntry[]> {
  const reader = entry.createReader();
  const entries: FileSystemEntry[] = [];
  for (;;) {
    const batch = await readEntryBatch(reader);
    if (batch.length === 0) return entries;
    entries.push(...batch);
  }
}

async function collectEntry(entry: FileSystemEntry, output: DroppedFile[]) {
  if (entry.isFile) {
    const file = await readFileEntry(entry as FileSystemFileEntry);
    const name = normalizeEntryPath(entry.fullPath) || file.name;
    output.push({ file, name });
    return;
  }
  if (!entry.isDirectory) return;
  const children = await readDirectoryEntries(entry as FileSystemDirectoryEntry);
  for (const child of children) {
    try {
      await collectEntry(child, output);
    } catch {
      // Skip entries that cannot be read and continue with the rest.
    }
  }
}

function filesFromList(dataTransfer: DataTransfer): DroppedFile[] {
  return Array.from(dataTransfer.files ?? []).map((file) => ({
    file,
    name: (file as FileWithRelativePath).webkitRelativePath || file.name,
  }));
}

export async function collectDroppedFiles(
  dataTransfer: DataTransfer,
): Promise<DroppedFile[]> {
  let items: DataTransferItem[] = [];
  try {
    items = Array.from(dataTransfer.items ?? []);
  } catch {
    items = [];
  }

  const entries: FileSystemEntry[] = [];
  for (const item of items) {
    if (item.kind !== 'file') continue;
    const entry = entryForItem(item);
    if (entry) entries.push(entry);
  }

  if (entries.length === 0) return filesFromList(dataTransfer);

  const output: DroppedFile[] = [];
  for (const entry of entries) {
    try {
      await collectEntry(entry, output);
    } catch {
      // Keep any files already collected from other dropped entries.
    }
  }
  return output;
}
