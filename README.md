# Recording Analyzer Lambda Function (Rust)

Esta función de AWS Lambda procesa archivos de audio utilizando la API de **Google Gemini** para generar transcripciones, resúmenes ejecutivos, extracción de tópicos, action items y highlights de forma automatizada.

## Arquitectura

El proceso se divide en un pipeline modular:
1. **Transcripción**: Conversión de audio a texto con marcas de tiempo.
2. **Análisis Paralelo**:
   - Extracción de Tópicos cronológicos.
   - Resumen Ejecutivo.
   - Identificación de Action Items (Dueños, Prioridad, Fechas).
   - Extracción de Highlights clave.

## Requisitos

- [Rust](https://www.rust-lang.org/) (Stable)
- [Cargo Lambda](https://www.cargo-lambda.info/)
- API Key de Google Gemini (AI Studio)

## Despliegue

El despliegue está automatizado mediante **GitHub Actions**. Al hacer push a la rama `master`, el workflow:
1. Compila el código para la arquitectura `arm64`.
2. Genera un binario optimizado para Amazon Linux 2023.
3. Actualiza el código de la función en AWS Lambda.

### Configuración de GitHub Secrets
Para que el despliegue funcione, el repositorio debe tener configurados los siguientes secretos:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `LAMBDA_FUNCTION_NAME`

## Desarrollo Local

Para compilar localmente para Lambda:
```bash
cargo lambda build --release --arm64
```

Para probar la lógica sin desplegar:
```bash
cargo run
```
