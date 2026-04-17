# Instrucciones del Proyecto (Gemini CLI)

Este archivo `GEMINI.md` contiene las directrices principales para el desarrollo y mantenimiento del proyecto `recording_analyzer_lambda_function`. Como agente de IA, debes seguir estas reglas al modificar o expandir la base de código.

## Arquitectura del Proyecto

El proyecto es una función de AWS Lambda escrita en Rust que procesa un archivo de audio mediante múltiples llamadas a la API de Gemini. 
La arquitectura se basa en un **Pipeline Modular**:
1. **Descarga y Carga**: El audio se descarga desde la URL proporcionada a `/tmp` y se carga en la **Gemini File API** para un procesamiento multimodal óptimo.
2. **Transcripción**: Convierte el audio en texto con marcas de tiempo usando un prompt especializado que optimiza el uso de tokens.
3. **Extracción de Tópicos**: Agrupa la conversación por temas cronológicos.
4. **Resumen Ejecutivo**: Genera un resumen de alto nivel.
5. **Action Items**: Extrae las tareas pendientes con dueños, prioridad y fechas.
6. **Highlights**: Resalta los momentos o citas clave de la llamada.

Las etapas 3 a 6 se ejecutan **en paralelo** aprovechando `tokio::try_join!` y el hecho de que son independientes entre sí (todas dependen de la misma transcripción original generada en el paso 2).

## Reglas de Desarrollo

- **Tipado Fuerte**: Todo el JSON devuelto o enviado por Gemini debe ser serializado y deserializado mediante estructuras definidas en `src/models.rs` usando `serde`.
- **Manejo de Errores**: Utilizar el enumerador `AppError` en `src/error.rs` impulsado por `thiserror`. Evita usar `.unwrap()` o `.expect()` en código de producción; propaga los errores con `?`.
- **Llamadas a la API**: Toda la comunicación con Gemini debe pasar por la estructura `GeminiClient` en `src/gemini.rs`, la cual incluye reintentos automáticos (exponential backoff) en caso de fallos de red o errores HTTP, así como protección mediante timeouts y serialización asegurada vía JSON Schema.
- **Asincronía**: Usa `tokio` para la concurrencia. Evita el bloqueo del hilo de ejecución en tareas de E/S.
- **Modificación de Prompts**: Si se modifica un prompt en `src/pipeline.rs`, asegúrate de mantener la instrucción explícita de devolver un JSON estructurado para aprovechar la propiedad `response_mime_type: "application/json"`.

## Próximas Iteraciones (Tareas Pendientes a considerar)

- **Diarización Mejorada**: Integrar herramientas de Speech-to-Text más específicas para la transcripción (ej., Whisper) si la calidad de la diarización de Gemini nativa no es suficiente.
- **Tests**: Añadir tests unitarios o tests de integración simulando las respuestas de la API (`wiremock` o interfaces mockeadas de la estructura del Pipeline).
- **Parámetros del Payload**: Permitir personalizar a nivel petición (payload del event) aspectos del resumen o los requerimientos para los highlights.

## Comandos Útiles

- Construir el proyecto localmente: `cargo build`
- Compilar para AWS Lambda: `cargo lambda build --release` (Requiere Cargo Lambda)
- Ejecutar linter: `cargo clippy -- -D warnings`
- Dar formato al código: `cargo fmt`
