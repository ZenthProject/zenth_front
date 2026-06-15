import { invoke } from '@tauri-apps/api/core';

export interface FileAnalysis {
  file_type: string;
  size: number;
  signature_valid: boolean;
  extension_matches_signature: boolean;
  error?: string;
}

export interface SanitizeResponse {
  success: boolean;
  message: string;
  sanitized_data?: number[];
  original_size: number;
  sanitized_size: number;
}

export class FileService {
  /**
   * Analyse un fichier sans le modifier
   */
  static async analyzeFile(filePath: string, fileData: Uint8Array): Promise<FileAnalysis> {
    return await invoke<FileAnalysis>('analyze_file', {
      filePath,
      fileData: Array.from(fileData),
    });
  }

  /**
   * Sanitize un fichier
   */
  static async sanitizeFile(filePath: string, fileData: Uint8Array): Promise<SanitizeResponse> {
    return await invoke<SanitizeResponse>('sanitize_file', {
      filePath,
      fileData: Array.from(fileData),
    });
  }

  /**
   * Obtient les formats supportés
   */
  static async getSupportedFormats(): Promise<string[]> {
    return await invoke<string[]>('get_supported_formats');
  }

  /**
   * Lit un fichier et retourne ses données
   */
  static async readFile(file: File): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = (e) => {
        const arrayBuffer = e.target?.result as ArrayBuffer;
        resolve(new Uint8Array(arrayBuffer));
      };
      reader.onerror = reject;
      reader.readAsArrayBuffer(file);
    });
  }

  /**
   * Télécharge un fichier sanitizé
   */
  static downloadFile(data: number[], filename: string) {
    const blob = new Blob([new Uint8Array(data)]);
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }
}
