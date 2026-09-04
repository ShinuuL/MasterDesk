import { useState } from "react";
import type { Note } from "../types";
import * as api from "../api";
import { Modal } from "./Modal";
import { noteSwatch } from "../theme/noteSurface";

/** Mesma paleta do `NoteCard` — a cor escolhida aqui é a que a nota já nasce. */
const COLORS = [
  "#FFEB3B",
  "#FF9800",
  "#8BC34A",
  "#03A9F4",
  "#E91E63",
  "#9C27B0",
  "#FFFFFF",
  "#263238",
] as const;

interface Props {
  onClose: () => void;
  /** Recebe a nota criada, para o quadro inseri-la sem recarregar tudo. */
  onCreated: (note: Note) => void;
}

/**
 * Criação de nota em diálogo.
 *
 * ## Por que saiu da barra de campos
 *
 * O conteúdo era um `<input>` de uma linha: dava para escrever um título e
 * pouco mais. Nota é o lugar de escrever de verdade, e o campo antigo não
 * aceitava nem quebra de linha — o Enter criava a nota. Aqui é um `textarea`,
 * e a criação é um botão explícito.
 *
 * `Ctrl+Enter` ainda cria, para quem vinha do fluxo de teclado. `Enter` solto
 * não: dentro de um `textarea` ele é quebra de linha, e roubar isso seria pior
 * que perder o atalho.
 */
export function NoteFormModal({ onClose, onCreated }: Props) {
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [color, setColor] = useState<string>(COLORS[0]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSave = title.trim().length > 0;

  const handleSave = async () => {
    if (!canSave) return;
    setSaving(true);
    setError(null);
    try {
      const created = await api.createNote({ title: title.trim(), content, color });
      onCreated(created);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title="Nova nota"
      onClose={onClose}
      footer={
        <>
          {error && (
            <p className="md-modal-error" style={{ marginRight: "auto" }}>
              {error}
            </p>
          )}
          <button className="md-btn" onClick={onClose} disabled={saving}>
            Cancelar
          </button>
          <button
            className="md-primary md-primary-accent"
            onClick={() => void handleSave()}
            disabled={!canSave || saving}
            aria-disabled={!canSave || saving}
          >
            {saving ? "Salvando…" : "Criar nota"}
          </button>
        </>
      }
    >
      <div className="md-field">
        <label htmlFor="note-modal-title">Título</label>
        <input
          id="note-modal-title"
          className="md-input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Título da nota"
          maxLength={200}
          autoFocus
        />
      </div>

      <div className="md-field">
        <label htmlFor="note-modal-content">Conteúdo</label>
        <textarea
          id="note-modal-content"
          className="md-input"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Escreva à vontade — Ctrl+Enter cria a nota"
          rows={8}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void handleSave();
          }}
        />
      </div>

      <div className="md-field">
        <span className="md-eyebrow">Cor</span>
        <div className="md-swatches" role="group" aria-label="Escolher cor">
          {COLORS.map((c) => (
            <button
              key={c}
              type="button"
              className="md-swatch"
              onClick={() => setColor(c)}
              aria-pressed={c === color}
              aria-label={`Cor ${c}`}
              title={c}
              style={{ background: noteSwatch(c) }}
            />
          ))}
        </div>
      </div>
    </Modal>
  );
}
