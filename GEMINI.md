# Instrucciones del Proyecto (Gemini CLI)

Este archivo `GEMINI.md` contiene las directrices principales para el desarrollo y mantenimiento del proyecto `recording_analyzer_lambda_function`. Como agente de IA, debes seguir estas reglas al modificar o expandir la base de código.

## Arquitectura del Proyecto

El proyecto es una función de AWS Lambda escrita en Rust que procesa un archivo de audio mediante múltiples llamadas a la API de Gemini. 
La arquitectura se basa en un **Pipeline Modular**:
1. **Transcripción**: Convierte el audio (pasado por URL) en texto con marcas de tiempo usando un prompt especializado.
2. **Extracción de Tópicos**: Agrupa la conversación por temas cronológicos.
3. **Resumen Ejecutivo**: Genera un resumen de alto nivel.
4. **Action Items**: Extrae las tareas pendientes con dueños, prioridad y fechas.
5. **Highlights**: Resalta los momentos o citas clave de la llamada.

Las etapas 2 a 5 se ejecutan **en paralelo** aprovechando `tokio::try_join!` y el hecho de que son independientes entre sí (todas dependen de la misma transcripción original generada en el paso 1).

## Reglas de Desarrollo

- **Tipado Fuerte**: Todo el JSON devuelto o enviado por Gemini debe ser serializado y deserializado mediante estructuras definidas en `src/models.rs` usando `serde`.
- **Manejo de Errores**: Utilizar el enumerador `AppError` en `src/error.rs` impulsado por `thiserror`. Evita usar `.unwrap()` o `.expect()` en código de producción; propaga los errores con `?`.
- **Llamadas a la API**: Toda la comunicación con Gemini debe pasar por la estructura `GeminiClient` en `src/gemini.rs`, la cual incluye reintentos automáticos (exponential backoff) en caso de fallos de red o errores HTTP, así como protección mediante timeouts y serialización asegurada vía JSON Schema.
- **Asincronía**: Usa `tokio` para la concurrencia. Evita el bloqueo del hilo de ejecución en tareas de E/S.
- **Modificación de Prompts**: Si se modifica un prompt en `src/pipeline.rs`, asegúrate de mantener la instrucción explícita de devolver un JSON estructurado para aprovechar la propiedad `response_mime_type: "application/json"`.

## Próximas Iteraciones (Tareas Pendientes a considerar)

- **Soporte S3 Real**: Actualmente, la URL del archivo de audio es enviada a Gemini como contexto para la transcripción. Para un sistema real con grabaciones en S3 sin acceso público, puede ser necesario descargar el archivo utilizando el SDK oficial de AWS (`aws-sdk-s3`), almacenarlo en la carpeta efímera `/tmp` de la Lambda, y usar la File API de Gemini para la ingesta de archivos.
- **Diarización Mejorada**: Integrar herramientas de Speech-to-Text más específicas para la transcripción (ej., Whisper) si la calidad de la diarización de Gemini nativa con URLs no es suficiente.
- **Tests**: Añadir tests unitarios o tests de integración simulando las respuestas de la API (`wiremock` o interfaces mockeadas de la estructura del Pipeline).
- **Parámetros del Payload**: Permitir personalizar a nivel petición (payload del event) aspectos del resumen o los requerimientos para los highlights.

## Comandos Útiles

- Construir el proyecto localmente: `cargo build`
- Compilar para AWS Lambda: `cargo lambda build --release` (Requiere Cargo Lambda)
- Ejecutar linter: `cargo clippy -- -D warnings`
- Dar formato al código: `cargo fmt`
