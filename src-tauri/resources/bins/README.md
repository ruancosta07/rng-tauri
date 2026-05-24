# Windows Binaries

Esta pasta deve conter os executáveis para Windows antes de fazer o build.
No Linux/macOS esses arquivos são ignorados (o app usa as ferramentas do sistema).

## Estrutura esperada

```
bins/
  pdftoppm.exe
  pdftotext.exe
  tesseract.exe
  tessdata/
    eng.traineddata
  (+ todas as DLLs necessárias no mesmo nível dos .exe)
```

## Como obter

### Poppler (pdftoppm + pdftotext)
1. Acesse: https://github.com/oschwartz10612/poppler-windows/releases/latest
2. Baixe o arquivo `Release-XX.XX.X-0.zip`
3. Extraia e copie para esta pasta:
   - `Library/bin/pdftoppm.exe`
   - `Library/bin/pdftotext.exe`
   - Todas as `.dll` da pasta `Library/bin/`

### Tesseract
1. Acesse: https://github.com/UB-Mannheim/tesseract/wiki
2. Baixe o instalador e instale, ou use a versão portátil
3. Copie para esta pasta:
   - `tesseract.exe`
   - Todas as `.dll` do diretório de instalação
4. Copie o arquivo de idioma:
   - `tessdata/eng.traineddata`
   - (o arquivo fica em `C:\Program Files\Tesseract-OCR\tessdata\eng.traineddata` após instalar)

## Verificação

Após colocar os arquivos, o build do Windows incluirá tudo automaticamente.
O app em runtime buscará os executáveis em `<install_dir>/bins/`.
