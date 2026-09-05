# Publica uma versao do MasterNote no repo de releases privado.
#
# Uma release do MasterNote carrega DOIS manifestos, porque tem dois publicos:
#
#   masternote.stable.manifest.json  -> assinado Ed25519 pelo deploy-base.
#                                       O PORTAL le isso para montar o botao
#                                       de download.
#   latest.json                      -> dialeto do updater do Tauri, com a
#                                       assinatura minisign do .nsis.zip.
#                                       O APP INSTALADO le isso (atraves da
#                                       rota /updater.json do gateway, que
#                                       reescreve as URLs para si mesmo).
#
# Publicar so um dos dois deixa metade do caminho funcionando em silencio: o
# portal sem botao, ou o app sem atualizacao. Por isso os dois saem daqui juntos.
#
# Uso:
#   pwsh scripts/release.ps1 -Version 0.2.0
#   pwsh scripts/release.ps1 -Version 0.2.0 -DryRun

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    # Repassado ao deploy-base: mostra o manifesto assinado e nao envia nada.
    # O build acontece de qualquer forma -- e ele que produz o que sera inspecionado.
    [switch]$DryRun,

    [string]$NotesFile,

    [string]$SigningKey = 'C:/chaves/masternote-updater.key',

    [string]$DeployBase = '../deploy-base'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

# --- 1. Coerencia de versao -------------------------------------------------
#
# A `version` do tauri.conf.json vai compilada dentro do binario, e o updater
# compara a versao do manifesto com ela. Publicar 0.3.0 com o app compilado como
# 0.2.0 faz cada instalacao se achar desatualizada contra ela mesma e mostrar o
# toast em looping, sem nunca convergir.

$confPath = Join-Path $raiz 'src-tauri/tauri.conf.json'
$conf = Get-Content $confPath -Raw | ConvertFrom-Json
if ($conf.version -ne $Version) {
    throw "versao divergente: tauri.conf.json diz '$($conf.version)', voce pediu '$Version'. Alinhe os dois (e o Cargo.toml do workspace) antes de publicar."
}

$cargoVersao = (Select-String -Path (Join-Path $raiz 'Cargo.toml') -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
if ($cargoVersao -ne $Version) {
    throw "versao divergente: Cargo.toml do workspace diz '$cargoVersao', voce pediu '$Version'."
}

if (-not (Test-Path $SigningKey)) {
    throw "chave minisign nao encontrada em '$SigningKey'. Sem ela o .nsis.zip sai sem assinatura e o updater rejeita a atualizacao. Gere com: frontend/node_modules/.bin/tauri signer generate -w $SigningKey"
}

# --- 2. Build ---------------------------------------------------------------
#
# `createUpdaterArtifacts: true` no tauri.conf.json faz o build produzir, alem
# do instalador, o par .nsis.zip + .nsis.zip.sig que o updater consome.

Write-Host "==> compilando MasterNote $Version" -ForegroundColor Cyan

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $SigningKey -Raw

# A chave nao tem senha, e e `CI` que faz o Tauri aceitar isso sem perguntar.
#
# Nao adianta tentar `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''`: no
# PowerShell, atribuir string vazia a uma variavel de ambiente REMOVE a
# variavel em vez de defini-la como vazia. O Tauri entao nao encontra senha
# nenhuma e abre um prompt interativo -- que, rodando sem console anexado,
# trava o build indefinidamente depois de ja ter compilado tudo.
$env:CI = 'true'

$tauriCli = Join-Path $raiz 'frontend/node_modules/@tauri-apps/cli/tauri.js'
if (-not (Test-Path $tauriCli)) {
    throw "CLI do Tauri nao encontrada. Rode `npm ci` em frontend/."
}

# O frontend e compilado aqui, explicitamente, e o `beforeBuildCommand` do
# tauri.conf.json e neutralizado no lugar de ser usado.
#
# Motivo: aquele hook e `npm run build --prefix ../frontend`, e o CLI do Tauri o
# executa a partir da RAIZ do projeto (`MasterDesk/`), nao de `src-tauri/`. O
# `../frontend` entao aponta para fora do repositorio e o build morre com um
# ENOENT que nao diz nada sobre a causa. A partir da raiz o certo seria
# `--prefix frontend` -- mas o mesmo campo e usado pelo `beforeDevCommand` no
# fluxo de desenvolvimento, que hoje funciona com `../frontend`. Corrigir um
# quebraria o outro, entao o script nao depende de nenhum dos dois.

Write-Host "==> compilando o frontend" -ForegroundColor Cyan
npm run build --prefix (Join-Path $raiz 'frontend')
if ($LASTEXITCODE -ne 0) { throw "o build do frontend falhou (exit $LASTEXITCODE)" }

$overridePath = Join-Path ([System.IO.Path]::GetTempPath()) 'masternote-build-override.json'
[System.IO.File]::WriteAllText(
    $overridePath,
    '{"build":{"beforeBuildCommand":""}}',
    (New-Object System.Text.UTF8Encoding $false)
)

$cwdAnterior = [Environment]::CurrentDirectory
Push-Location (Join-Path $raiz 'src-tauri')
[Environment]::CurrentDirectory = (Get-Location).ProviderPath
try {
    node $tauriCli build --config $overridePath
    if ($LASTEXITCODE -ne 0) { throw "o build do Tauri falhou (exit $LASTEXITCODE)" }
}
finally {
    Pop-Location
    [Environment]::CurrentDirectory = $cwdAnterior
    Remove-Item $overridePath -ErrorAction SilentlyContinue
    # Nao deixa a chave privada pendurada no ambiente da sessao depois do build.
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:CI -ErrorAction SilentlyContinue
}

# --- 3. Localiza os artefatos ------------------------------------------------

$bundle = Join-Path $raiz 'target/release/bundle/nsis'
$produto = $conf.productName

$instalador = Join-Path $bundle "${produto}_${Version}_x64-setup.exe"

# A partir do Tauri 2.11 nao ha `.nsis.zip`: com `createUpdaterArtifacts: true`
# o proprio instalador e o pacote de atualizacao, e o build deixa um `.sig` ao
# lado dele. Um unico arquivo serve aos dois publicos -- o portal baixa o mesmo
# binario que o updater instala.
$updaterSig = "$instalador.sig"

foreach ($f in @($instalador, $updaterSig)) {
    if (-not (Test-Path $f)) { throw "artefato esperado nao saiu do build: $f" }
}

# O deploy-base versiona o asset a partir do nome do arquivo, entao o instalador
# entra com um nome estavel e sai como masternote-$Version.exe.
$distWin = Join-Path $raiz 'dist-windows'
New-Item -ItemType Directory -Force -Path $distWin | Out-Null
Copy-Item $instalador (Join-Path $distWin 'masternote.exe') -Force

# --- 4. latest.json ----------------------------------------------------------
#
# A `url` aponta direto para a rota de download do gateway. O /updater.json
# reescreve esse campo de qualquer forma, mas deixa-lo ja correto significa que
# o arquivo tambem serve para depurar fora do gateway -- e o basename precisa
# bater com o nome do asset na release, que e como o gateway o encontra.
#
# Note que o nome aqui e o do asset RENOMEADO pelo deploy-base
# (`masternote-0.2.0.exe`), nao o do arquivo que saiu do build
# (`MasterNote_0.2.0_x64-setup.exe`). Sao o mesmo binario, e e por isso que uma
# copia so basta: a assinatura minisign cobre os BYTES do arquivo, nao o nome
# dele -- o nome que aparece no comentario do `.sig` e informativo.

$gateway = 'https://updates-gateway.sofaltaumaletr.workers.dev'
$assetNome = "masternote-$Version.exe"

$notas = ''
if ($NotesFile) { $notas = Get-Content $NotesFile -Raw }

$latest = [ordered]@{
    version   = $Version
    notes     = $notas
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content $updaterSig -Raw).Trim()
            url       = "$gateway/v1/apps/masternote/download/$Version/$assetNome"
        }
    }
}

$latestPath = Join-Path $distWin 'latest.json'

# Escrito SEM BOM, de proposito. `Out-File -Encoding utf8` no Windows PowerShell
# 5.1 emite UTF-8 com BOM, e o gateway le este arquivo com `JSON.parse` -- que
# estoura em `﻿` antes da primeira chave. O erro so apareceria em producao,
# como um 502 do Worker, com o build local tendo passado.
$semBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($latestPath, ($latest | ConvertTo-Json -Depth 5), $semBom)
Write-Host "==> latest.json escrito em $latestPath" -ForegroundColor Cyan

# --- 5. Publica --------------------------------------------------------------

$python = Join-Path $raiz "$DeployBase/.venv/Scripts/python.exe"
if (-not (Test-Path $python)) {
    throw "venv do deploy-base nao encontrada em '$python'. Ajuste -DeployBase."
}

$argumentos = @('-m', 'deploybase.cli', 'publish', $Version)
if ($NotesFile) { $argumentos += @('--notes-file', $NotesFile) }
if ($DryRun) { $argumentos += '--dry-run' }

Write-Host "==> publicando via deploy-base" -ForegroundColor Cyan
& $python @argumentos
if ($LASTEXITCODE -ne 0) { throw "o publish do deploy-base falhou (exit $LASTEXITCODE)" }

if ($DryRun) {
    Write-Host "`n--dry-run: nada foi enviado. latest.json e o instalador ficaram em dist-windows/." -ForegroundColor Yellow
    exit 0
}

# --- 6. Anexa o manifesto do updater -----------------------------------------
#
# So o latest.json: o binario ja subiu pelo [[artifact]] do deploy.toml, e a
# assinatura viaja DENTRO deste JSON, no campo `signature` -- o updater nao
# busca um `.sig` separado.
#
# Fica fora do [[artifact]] de proposito: o deploy-base renomeia o que passa por
# ali para incluir a versao, e um `latest-0.2.0.json` nao seria encontrado pela
# rota /updater.json, que procura o asset pelo nome exato `latest.json`.

$tag = "masternote-v$Version"
Write-Host "==> anexando latest.json em $tag" -ForegroundColor Cyan
gh release upload $tag $latestPath --repo 'ShinuuL/Releases' --clobber
if ($LASTEXITCODE -ne 0) { throw "falha ao subir o latest.json (exit $LASTEXITCODE)" }

Write-Host "`nPublicado: $tag" -ForegroundColor Green
Write-Host "  portal  : $gateway/v1/apps/masternote/latest"
Write-Host "  updater : $gateway/v1/apps/masternote/updater.json"
