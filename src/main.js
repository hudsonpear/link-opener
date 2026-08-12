const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const fileInput = document.getElementById('fileInput');
const selectFileBtn = document.getElementById('selectFileBtn');
const openLinksBtn = document.getElementById('openLinksBtn');
const debugConsole = document.getElementById('debugConsole');
const dragOverlay = document.getElementById('dragOverlay');
const linksList = document.getElementById('linksList');
const selectAllBtn = document.getElementById('selectAllBtn');
const unselectAllBtn = document.getElementById('unselectAllBtn');
const copySelectedBtn = document.getElementById('copySelectedBtn');

let selectedFilePath = null;
let fileContent = null;
let fileType = null;

const allowedExtensions = [
  'txt', 'docx', 'doc', 'xlsx', 'xls', 'xlsm', 'xlsb', 'ods', 'pptx', 'csv', 'pdf',
  'html', 'htm', 'md', 'json', 'xml', 'yaml', 'yml', 'ini', 'log', 'css', 'js', 'ts', 'rtf',
];

// Matches URLs embedded anywhere in text, not just whole-line matches.
const URL_REGEX = /(?:https?:\/\/|ftp:\/\/|www\.)[^\s<>"')\]]+|mailto:[^\s<>"')\]]+/gi;

// ---------------- Drag & Drop (native Tauri file drop) ----------------

getCurrentWindow().onDragDropEvent((event) => {
  const { type } = event.payload;

  if (type === 'enter' || type === 'over') {
    dragOverlay.style.display = 'flex';
  } else if (type === 'leave') {
    dragOverlay.style.display = 'none';
  } else if (type === 'drop') {
    dragOverlay.style.display = 'none';
    const paths = event.payload.paths;
    if (!paths || paths.length === 0) return;

    const filePath = paths[0];
    const ext = filePath.split('.').pop().toLowerCase();

    if (!allowedExtensions.includes(ext)) {
      logConsole(`Unsupported file type: ${ext}\n`);
      return;
    }

    loadFile(filePath);
  }
});

// ---------------- File Loader ----------------
async function loadFile(filePath) {
  if (!filePath || typeof filePath !== "string") {
    logConsole('[ERROR] Invalid file path.\n');
    return;
  }
  try {
    const result = await invoke('read_file', { path: filePath });

    if (!result || !result.text) {
      logConsole(`\n[WARN] No content returned for: ${filePath}\n`);
      return null;
    }

    selectedFilePath = filePath;
    fileContent = result.text;
    fileType = result.type?.toUpperCase() || 'UNKNOWN';

    const fileName = filePath.split(/[\\/]/).pop();
    fileInput.value = filePath;

    logConsole(`\n--- File Loaded ---\n`);
    logConsole(`File: ${fileName}\n`);
    logConsole(`Path: ${filePath}\n`);
    logConsole(`Type: ${fileType}\n`);
    if (result.size) logConsole(`Size: ${formatFileSize(result.size)}\n`);

    const lineCount = fileContent.split(/\r?\n/).length;
    logConsole(`Lines: ${lineCount}\n`);

    const links = extractLinksFromContent(fileContent, fileType);
    logConsole(`Links detected: ${links.length}\n`);

    renderLinks(links, fileType);
    logConsole(`--- Done processing file ---\n`);

    return result;
  }
  catch (error) {
    logConsole(`\n[ERROR] Failed to read file: ${error.message || error}\n`);
    return null;
  }
}

function extractLinksFromContent(text, type) {
  const links = (type === 'HTML' || type === 'HTM')
    ? extractLinksFromHTML(text)
    : extractUrlsFromText(text);

  return Array.from(new Set(links)); // dedup, preserve order
}

// ---------------- Helpers ----------------

function extractUrlsFromText(text) {
  const matches = text.match(URL_REGEX) || [];
  return matches.map(cleanUrl).filter(Boolean);
}

function cleanUrl(raw) {
  // strip trailing punctuation picked up from surrounding prose (e.g. "...example.com.")
  const url = raw.trim().replace(/[.,;:!?)\]"'>]+$/, '');
  if (!url) return null;
  return /^www\./i.test(url) ? 'https://' + url : url;
}

function extractLinksFromHTML(html) {
  const links = [];
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, 'text/html');
  doc.querySelectorAll('a[href]').forEach(a => links.push(a.href));
  links.push(...extractUrlsFromText(doc.body ? doc.body.textContent : html));
  return links;
}

function isValidUrl(string) {
  const urlPattern = /^(https?:\/\/|ftp:\/\/|www\.|mailto:)\S+$/i;
  return urlPattern.test(string.trim());
}

function formatFileSize(bytes) {
  if (bytes < 1024) return bytes + ' bytes';
  if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + ' KB';
  return Math.round(bytes / (1024 * 1024)) + ' MB';
}

function logConsole(text) {
  debugConsole.value += text;
  debugConsole.scrollTop = debugConsole.scrollHeight;
}

// ---------------- Render Links ----------------
function renderLinks(links, fileType) {
  linksList.innerHTML = '';
  const container = document.getElementById('linksContainer');

  if (!links || links.length === 0) {
    linksList.innerHTML = '<li>No links found in this file.</li>';
    container.style.display = 'none';
    return;
  }

  container.style.display = 'block';

  links.forEach((link, index) => {
    const li = document.createElement('li');
    li.style.display = 'flex';
    li.style.alignItems = 'center';
    li.style.marginBottom = '5px';

    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = true;
    checkbox.id = `link-${index}`;

    const label = document.createElement('label');
    label.setAttribute('for', `link-${index}`);
    label.textContent = link;
    label.style.marginLeft = '8px';

    li.appendChild(checkbox);
    li.appendChild(label);
    linksList.appendChild(li);
  });

  logConsole(`Found ${links.length} link(s) in ${fileType} file\n`);
}

// ---------------- Buttons ----------------

// File selection button
selectFileBtn.addEventListener('click', async () => {
  try {
    const filePath = await invoke('open_file_dialog');
    if (filePath) {
      await loadFile(filePath);
    }
  } catch (error) {
    logConsole(`Error selecting file: ${error.message || error}\n`);
  }
});

// Open selected links
openLinksBtn.addEventListener('click', async () => {
  try {
    const checkboxes = linksList.querySelectorAll('input[type="checkbox"]:checked');
    if (checkboxes.length === 0) {
      logConsole('No links selected to open.\n');
      return;
    }

    logConsole(`Opening ${checkboxes.length} selected link(s)...\n`);

    let opened = 0;
    for (const cb of checkboxes) {
      let link = cb.nextSibling.textContent.trim();
      if (/^www\./i.test(link)) link = 'https://' + link;
      if (!isValidUrl(link)) {
        logConsole(`Skipped invalid link: ${link}\n`);
        continue;
      }
      try {
        await invoke('open_link', { url: link });
        logConsole(`Opened: ${link}\n`);
        opened++;
      }
      catch (err) { logConsole(`Failed to open ${link}: ${err.message || err}\n`); }
    }
    logConsole(`Finished. Successfully opened ${opened} link(s).\n`);
  }
  catch (error) { logConsole(`Error while opening links: ${error.message || error}\n`); }
});

// Select / Unselect all
selectAllBtn.addEventListener('click', () => { linksList.querySelectorAll('input[type="checkbox"]').forEach(cb => cb.checked = true); });
unselectAllBtn.addEventListener('click', () => { linksList.querySelectorAll('input[type="checkbox"]').forEach(cb => cb.checked = false); });

// Copy selected links
copySelectedBtn.addEventListener('click', async () => {
  const checkboxes = linksList.querySelectorAll('input[type="checkbox"]:checked');
  const selectedLinks = Array.from(checkboxes).map(cb => cb.nextSibling.textContent);

  if (selectedLinks.length === 0) {
    logConsole('No links selected to copy.\n');
    return;
  }

  try {
    await navigator.clipboard.writeText(selectedLinks.join('\n'));
    logConsole(`Copied ${selectedLinks.length} link(s) to clipboard.\n`);
  }
  catch (err) {
    logConsole(`Failed to copy links: ${err.message || err}\n`);
  }
});

// ---------------- Open from OS (file association / "Open With") ----------------

// Already-running instance re-triggered via a second launch with a file arg
listen('open-file-from-os', async (event) => {
  const filePath = event.payload;
  if (!filePath || typeof filePath !== "string" || filePath.trim() === "") return;

  logConsole(`\n--- Opened From Windows ---\n`);
  logConsole(`File: ${filePath}\n`);
  await loadFile(filePath);
  logConsole(`Auto-loaded file from Windows.\n`);
});

// First launch with a file arg (double-click / "Open With")
(async () => {
  try {
    const startupFile = await invoke('get_startup_file');
    if (startupFile) {
      logConsole(`\n--- Opened From Windows ---\n`);
      logConsole(`File: ${startupFile}\n`);
      await loadFile(startupFile);
      logConsole(`Auto-loaded file from Windows.\n`);
    }
  } catch (err) {
    logConsole(`Failed to read startup file: ${err.message || err}\n`);
  }
})();
