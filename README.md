# Recording Analyzer Lambda Function (Rust)

Esta función de AWS Lambda procesa archivos de audio utilizando **Gemini en Vertex AI** para generar transcripciones, resúmenes ejecutivos, extracción de tópicos, action items y highlights de forma automatizada.

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
- Service account de Google Cloud con permisos para usar Vertex AI

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

La función Lambda debe tener configuradas estas variables de entorno:
- `GOOGLE_SERVICE_ACCOUNT_JSON`: JSON completo del service account de Google Cloud. Recomendado para Lambda, idealmente cargado desde Secrets Manager.
- `VERTEX_AI_PROJECT_ID`: opcional si el JSON ya contiene el `project_id`.
- `VERTEX_AI_LOCATION`: opcional, por defecto `global`.
- `VERTEX_AI_MODEL`: opcional, por defecto `gemini-3-flash-preview`.

Para desarrollo local también se puede usar `GOOGLE_APPLICATION_CREDENTIALS` apuntando al archivo JSON del service account.

## Desarrollo Local

Para compilar localmente para Lambda:
```bash
cargo lambda build --release --arm64
```

Para probar la lógica sin desplegar:
```bash
cargo run
```
