// Sem janela de console no build de release.
//
// O padrão do Rust no Windows é o subsistema "console", que abre um prompt
// preto atrás da janela do app. Em ferramenta de linha de comando isso é
// correto; num app de bandeja é ruído que o usuário fecha por engano —
// fechar o console mata o processo.
//
// `not(debug_assertions)` mantém o console em `dev`, onde ele é útil: é para
// onde vão os `println!` e o `RUST_BACKTRACE`. Em release essa saída passa a
// não ter destino, o que é aceitável porque nada em release depende de
// `println!` para funcionar.
//
// Só tem efeito no Windows; nos outros alvos o atributo é ignorado.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    masterdesk_lib::run();
}
