/**
 * Génère les icônes de l'application dans src-tauri/icons/ (`npm run icons`).
 *
 * - Toutes les tailles viennent de src/assets/logo/atlas-icone.svg, rendu
 *   par `tauri icon` (la CLI de Tauri lit le SVG elle-même : aucun outil de
 *   conversion supplémentaire).
 * - La taille 16 × 16 vient de atlas-icone-16.svg, dessiné pour cette
 *   taille : elle remplace l'entrée 16 du fichier .ico de Windows et donne
 *   16x16.png (icône 16 px du .deb).
 * - Seules les icônes pour ordinateur sont gardées (pas Android ni iOS).
 *
 * Le script relit le .ico produit et échoue si son entrée 16 × 16 n'est pas
 * exactement le rendu de atlas-icone-16.svg.
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const logoDir = path.join(root, "src/assets/logo");
const mainSvg = path.join(logoDir, "atlas-icone.svg");
const smallSvg = path.join(logoDir, "atlas-icone-16.svg");
const iconsDir = path.join(root, "src-tauri/icons");
const tauriCli = path.join(root, "node_modules/@tauri-apps/cli/tauri.js");

/** Fichiers produits par `tauri icon` gardés pour Windows et Linux. */
const DESKTOP_ICONS = [
  "32x32.png",
  "64x64.png",
  "128x128.png",
  "128x128@2x.png",
  "icon.png",
  "icon.ico",
  "icon.icns",
  "Square30x30Logo.png",
  "Square44x44Logo.png",
  "Square71x71Logo.png",
  "Square89x89Logo.png",
  "Square107x107Logo.png",
  "Square142x142Logo.png",
  "Square150x150Logo.png",
  "Square284x284Logo.png",
  "Square310x310Logo.png",
  "StoreLogo.png",
];

const PNG_SIGNATURE = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
]);

function tauriIcon(args) {
  const result = spawnSync(process.execPath, [tauriCli, "icon", ...args], {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    process.stderr.write(result.stdout + result.stderr);
    throw new Error(
      `tauri icon ${args.join(" ")} a échoué (code ${result.status}).`,
    );
  }
}

/** Dimensions lues dans l'en-tête IHDR d'un PNG. */
function pngSize(data) {
  if (!data.subarray(0, 8).equals(PNG_SIGNATURE))
    throw new Error("PNG attendu.");
  return { width: data.readUInt32BE(16), height: data.readUInt32BE(20) };
}

/** Entrées d'un fichier .ico : dimensions, champs du répertoire, données. */
function readIco(data) {
  if (data.readUInt16LE(0) !== 0 || data.readUInt16LE(2) !== 1) {
    throw new Error("Fichier .ico invalide.");
  }
  const count = data.readUInt16LE(4);
  const entries = [];
  for (let i = 0; i < count; i++) {
    const at = 6 + 16 * i;
    const size = data.readUInt32LE(at + 8);
    const offset = data.readUInt32LE(at + 12);
    entries.push({
      width: data[at] || 256,
      height: data[at + 1] || 256,
      colors: data[at + 2],
      planes: data.readUInt16LE(at + 4),
      bitCount: data.readUInt16LE(at + 6),
      image: data.subarray(offset, offset + size),
    });
  }
  return entries;
}

/** Fichier .ico à partir de ses entrées, dans le même ordre. */
function writeIco(entries) {
  const header = Buffer.alloc(6 + 16 * entries.length);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  let offset = header.length;
  entries.forEach((entry, i) => {
    const at = 6 + 16 * i;
    header[at] = entry.width >= 256 ? 0 : entry.width;
    header[at + 1] = entry.height >= 256 ? 0 : entry.height;
    header[at + 2] = entry.colors;
    header[at + 3] = 0;
    header.writeUInt16LE(entry.planes, at + 4);
    header.writeUInt16LE(entry.bitCount, at + 6);
    header.writeUInt32LE(entry.image.length, at + 8);
    header.writeUInt32LE(offset, at + 12);
    offset += entry.image.length;
  });
  return Buffer.concat([header, ...entries.map((entry) => entry.image)]);
}

function main() {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "atlas-icons-"));
  try {
    const mainOut = path.join(work, "main");
    const smallOut = path.join(work, "small");
    tauriIcon([mainSvg, "-o", mainOut]);
    tauriIcon([smallSvg, "-p", "16", "-o", smallOut]);

    const small = fs.readFileSync(path.join(smallOut, "16x16.png"));
    const { width, height } = pngSize(small);
    if (width !== 16 || height !== 16) {
      throw new Error(`16x16.png mesure ${width} × ${height}.`);
    }

    // Icône 16 × 16 du .ico remplacée par le rendu dédié.
    const entries = readIco(fs.readFileSync(path.join(mainOut, "icon.ico")));
    const entry16 = entries.find((entry) => entry.width === 16);
    if (!entry16) throw new Error("Le .ico produit n'a pas d'entrée 16 × 16.");
    entry16.image = small;
    entry16.bitCount = 32;

    fs.mkdirSync(iconsDir, { recursive: true });
    for (const name of DESKTOP_ICONS) {
      fs.copyFileSync(path.join(mainOut, name), path.join(iconsDir, name));
    }
    fs.writeFileSync(path.join(iconsDir, "icon.ico"), writeIco(entries));
    fs.writeFileSync(path.join(iconsDir, "16x16.png"), small);

    // Contrôle du .ico écrit.
    console.log("icon.ico :");
    for (const entry of readIco(
      fs.readFileSync(path.join(iconsDir, "icon.ico")),
    )) {
      const actual = pngSize(entry.image);
      if (actual.width !== entry.width || actual.height !== entry.height) {
        throw new Error(
          `Entrée ${entry.width} : image de ${actual.width} × ${actual.height}.`,
        );
      }
      const source =
        entry.width === 16
          ? entry.image.equals(small)
            ? "atlas-icone-16.svg"
            : null
          : "atlas-icone.svg";
      if (source === null) {
        throw new Error(
          "L'entrée 16 × 16 n'est pas le rendu de atlas-icone-16.svg.",
        );
      }
      console.log(
        `  ${entry.width} × ${entry.height}  PNG  ${entry.image.length} octets  (${source})`,
      );
    }
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

main();
