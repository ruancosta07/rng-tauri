# Windows Binaries

Esta pasta já contém todos os executáveis e modelos necessários para o build do Windows.
No Linux/macOS esses arquivos são ignorados (o app usa as ferramentas do sistema).

## Conteúdo incluído

- `pdftoppm.exe` / `pdftotext.exe` + DLLs do Poppler
- `tesseract.exe` + DLLs
- `tessdata/eng.traineddata` (inglês)
- `tessdata/por.traineddata` (português)

O build do Windows inclui tudo automaticamente. O app em runtime buscará os executáveis em `<install_dir>/bins/`.
