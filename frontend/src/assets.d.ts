// Importação de imagem pelo Vite: o módulo exporta a URL final do arquivo.
declare module "*.png" {
  const url: string;
  export default url;
}
