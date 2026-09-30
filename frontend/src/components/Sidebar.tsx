import { ThemeToggle } from "./ThemeToggle";
import { StartupToggle } from "./StartupToggle";
// Cópia em 128 px de `src-tauri/icons/icon.png` (o ícone da janela e do
// instalador). Se o ícone mudar, regenere esta cópia junto.
import appIcon from "../assets/app-icon.png";

export type Section = "board" | "tickets" | "notes";

const SECTIONS: { id: Section; label: string; hint: string }[] = [
  { id: "board", label: "Quadro", hint: "Tarefas locais e do Mastersys" },
  { id: "tickets", label: "Chamados", hint: "Somente o que veio do Mastersys" },
  { id: "notes", label: "Notas", hint: "Post-its; destaque para ficar por cima" },
];

interface Props {
  section: Section;
  onSection: (s: Section) => void;
  username: string;
  onLogout: () => void;
}

/**
 * Barra lateral no lugar das abas do topo (redesenho de 2026-09-30).
 *
 * `nav` + botões com `aria-current`, e não `role="tablist"`: são seções de
 * navegação, não abas de um mesmo painel — e o tablist antigo cobria o `<nav>`
 * inteiro, marca e "Sair" incluídos (achado de acessibilidade da auditoria).
 */
export function Sidebar({ section, onSection, username, onLogout }: Props) {
  return (
    <nav className="md-sidebar" aria-label="Seções do MasterNote">
      <div className="md-sidebar-brand">
        {/* O ícone real do app, o mesmo da janela e do instalador. */}
        <img src={appIcon} alt="" width={32} height={32} className="md-sidebar-icon" />
        <div className="md-sidebar-brand-text">
          <span>MasterNote</span>
          <small title={username}>@{username}</small>
        </div>
      </div>

      <div className="md-sidebar-group">
        <span className="md-sidebar-caption">Trabalho</span>
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            type="button"
            className="md-sidebar-item"
            aria-current={section === s.id ? "page" : undefined}
            onClick={() => onSection(s.id)}
            title={s.hint}
          >
            {s.label}
          </button>
        ))}
      </div>

      <div className="md-sidebar-foot">
        <div className="md-sidebar-row">
          <span>Aparência</span>
          <ThemeToggle />
        </div>
        <div className="md-sidebar-row">
          <span>Iniciar com o Windows</span>
          <StartupToggle />
        </div>
        <button type="button" className="md-sidebar-item" onClick={onLogout}>
          Sair
        </button>
      </div>
    </nav>
  );
}
