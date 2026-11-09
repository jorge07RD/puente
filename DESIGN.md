---
name: Puente
description: "El móvil como instrumento bajo observación, rodeado de pequeños instrumentos de medida."
colors:
  accent: "#ff4a1f"
  ground: "#0a0a0b"
  card: "#141415"
  raised: "#1f1f21"
  edge: "#28282a"
  ink: "#f2f2ef"
  ink-soft: "#a6a6a1"
  dot-off: "#4a4a4c"
typography:
  value:
    fontFamily: "Martian Mono"
    fontSize: "20px"
    fontWeight: 400
  body:
    fontFamily: "Martian Mono"
    fontSize: "13px"
    fontWeight: 400
  label:
    fontFamily: "Martian Mono"
    fontSize: "12px"
    fontWeight: 500
    letterSpacing: "0.8px"
  control:
    fontFamily: "Martian Mono"
    fontSize: "12px"
    fontWeight: 500
rounded:
  card: "14px"
  screen: "16px"
  pill: "9999px"
spacing:
  item-y: "6px"
  gap: "10px"
  outer: "10px"
  phone-margin: "14px"
  padding: "16px"
  center-gutter: "24px"
  dot-pitch-min: "5px"
components:
  card:
    backgroundColor: "{colors.card}"
    rounded: "{rounded.card}"
    padding: "{spacing.padding}"
  pill:
    backgroundColor: "{colors.card}"
    textColor: "{colors.ink}"
    typography: "{typography.control}"
    rounded: "{rounded.pill}"
    padding: "0 14px"
    height: "32px"
  pill-hover:
    backgroundColor: "{colors.raised}"
  pill-active:
    backgroundColor: "{colors.edge}"
  pill-primary:
    backgroundColor: "{colors.card}"
    textColor: "{colors.accent}"
    typography: "{typography.control}"
    rounded: "{rounded.pill}"
    padding: "0 14px"
    height: "44px"
  pill-touch:
    height: "48px"
  icon-button:
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    size: "44px"
  icon-button-hover:
    backgroundColor: "{colors.raised}"
  icon-button-active:
    backgroundColor: "{colors.edge}"
  icon-button-primary:
    textColor: "{colors.accent}"
    rounded: "{rounded.pill}"
    size: "52px"
  switch:
    rounded: "{rounded.pill}"
    width: "34px"
    height: "18px"
---

# Design System: Puente

> UI nativa en Rust/egui, sin CSS ni DOM: el detector y las demás herramientas web de Impeccable no aplican aquí. La fuente normativa es `crates/puente-ui/src/theme.rs`. Las medidas son puntos lógicos de egui, escritos como `px` porque lo pide el formato.

## Overview

**Creative North Star: "El instrumento bajo observación"**

El móvil es lo observado y ocupa el centro. Alrededor, tarjetas pequeñas miden lo que le pasa: enlace, batería, audio, música, avisos, archivos. La ventana se lee como un banco de medida: fondo casi negro, tarjetas planas de filo fino, datos en puntos blancos y un único rojo anaranjado para lo que está activo, en vivo o pide atención.

La densidad es de instrumento y la tipografía es una sola familia monoespaciada. El mundo visual lo fijó la imagen de referencia del autor. El motivo propio es el puente de puntos entre la PC y el móvil, que se cierra al conectar y lleva un paquete rojo mientras llega vídeo.

**Key Characteristics:**
- Fondo casi negro con tarjetas planas y filo de 1 px.
- Gráficos de datos en matriz de puntos, nunca en barras o trazos continuos.
- Etiquetas numeradas, en mayúsculas y monoespaciadas.
- Un solo color, con significado fijo.
- Cada cifra sale de una medida real.

## Colors

Neutros casi sin tinte y un único acento saturado que aparece en pequeñas dosis.

### Primary
- **Rojo Señal** (accent): lo activo, lo que está en vivo y lo que pide atención. Punto de vivo en la cabecera, cabeza del anillo y de la barra de avance, columna más alta de la onda, paquete del puente, pulso del radar, interruptor encendido, batería baja (15 % o menos sin cargar), errores y controles primarios como reproducir, hacer sonar o compartir.

### Neutral
- **Fondo de Banco** (ground): el suelo de todas las pantallas.
- **Placa** (card): relleno de tarjetas y de píldoras en reposo.
- **Placa Alzada** (raised): relleno al pasar el puntero.
- **Filo** (edge): borde de tarjetas, píldoras y pantalla; relleno al pulsar.
- **Tinta** (ink): texto principal y puntos encendidos.
- **Tinta Suave** (ink-soft): etiquetas, unidades, texto secundario y borde al pasar el puntero.
- **Punto Apagado** (dot-off): puntos sin encender en anillos, puente, barra y zona de archivos.

### Named Rules
**La Regla del Único Color.** No hay segundo acento. El rojo significa activo, en vivo o atención; si un elemento no es ninguna de las tres cosas ni un control primario, no lo lleva. Única excepción, pedida por el autor: el verde `#4ade80` de las animaciones de carga (el puente mientras busca y la batería mientras se carga), que no aparece en ningún otro sitio.

**La Regla del Punto que Enciende.** Un gráfico de datos muestra su escala completa: puntos de Tinta encendidos, Punto Apagado para lo que falta y, en anillo y barra de avance, el último punto en Rojo Señal.

## Typography

**Body Font:** Martian Mono Regular, empaquetada en la app (con las fuentes de egui detrás para los caracteres que falten en las notificaciones).
**Label/Mono Font:** Martian Mono Medium.

**Character:** Una sola familia monoespaciada, de lectura técnica, en dos pesos. La jerarquía viene del tamaño, la caja y el dibujo en puntos, no de una pareja de fuentes.

### Hierarchy
- **Cifra en puntos** (matriz de 5×7, sin fuente): cifras principales, código de emparejamiento y palabra PUENTE. El tamaño lo da la distancia entre puntos: 5 como mínimo, 7 en la marca de escritorio, 10 en el código de escritorio, hasta 12 en el anillo de batería.
- **Value** (400, 20 px): medidas secundarias como fps y mbps.
- **Body** (400, 13 px): instrucciones, nombres y texto de avisos, en mayúsculas y minúsculas normales.
- **Label** (500, 12 px, tracking 0.8 px, MAYÚSCULAS, Tinta Suave): cabeceras numeradas y estados de tarjeta.
- **Control** (500, 12 px, MAYÚSCULAS): texto de píldoras e interruptores.

### Named Rules
**La Regla de la Etiqueta Numerada.** Cada tarjeta se titula con índice de dos dígitos y nombre (`01 ENLACE`). Los índices son consecutivos dentro de la pantalla; si una tarjeta no aparece, la siguiente se renumera.

**La Regla de la Matriz Corta.** La matriz de puntos es para cifras y palabras cortas, nunca para texto corrido, y no baja de 5 de distancia entre puntos. Las cifras tienen todas el mismo ancho, para que no bailen al cambiar.

**La Regla de la Cifra Medida.** Toda cifra viene de la conexión o del móvil. Sin dato se muestra un guion, nunca un cero ni un valor de ejemplo.

## Layout

La ventana de escritorio deja un margen exterior de 10 y separa las tarjetas por 10. Con móvil vertical hay tres zonas: cuatro tarjetas a la izquierda, el móvil con su barra de 56 en el centro y cuatro a la derecha. Las columnas miden entre 248 y 340 y se acercan al móvil hasta 24 de él; lo que sobra queda en los bordes de la ventana.

- **Columna izquierda:** cuatro tarjetas de igual alto.
- **Columna derecha:** Música (156), Avisos (el resto), Archivos (176) y Ajustes (128).
- **Móvil horizontal:** columna derecha de 248 y fila inferior de cuatro tarjetas de 168.
- **Ventana menor de 860 × 580:** una sola cosa cada vez, el móvil o las tarjetas apiladas (196 cada una). F2 alterna; en ventana grande oculta las tarjetas y deja solo la transmisión.
- **Móvil (app):** columna única con margen de 14 más la zona segura del sistema. Las tarjetas crecen con su contenido y la escala de letra de Android se respeta entre 1.0 y 1.4. Los objetivos táctiles miden 48.

**La Regla del Móvil al Centro.** La pantalla compartida conserva sus proporciones y ninguna tarjeta la invade. Las tarjetas ceden contenido antes que espacio al móvil.

## Elevation & Depth

Sistema plano. La profundidad la da el paso tonal de Fondo de Banco a Placa y el filo de 1 px. Ni tarjetas ni controles llevan sombra. La excepción es un residuo, no una regla: los tooltips heredan la sombra por defecto de egui.

**La Regla de la Placa Plana.** Sin sombras, resplandores ni rellenos en degradado. Los puntos sí pueden atenuarse por escalones, como los anillos del radar o los tirantes del puente; eso es dibujo en puntos, no degradado.

## Shapes

Tres formas conviven: la placa redondeada (14), la pantalla del móvil apenas más redonda (16) y el círculo o la píldora de los controles (radio igual a la mitad de la altura). Los bordes son siempre de 1 px; solo el aro del control primario redondo y el foco suben a 1.5. El punto es la unidad gráfica: su radio es 0.34 de la distancia entre puntos.

## Components

### Buttons
Controles de instrumento, delineados y discretos. Ninguno se rellena con color.

- **Shape:** píldora (radio de media altura), con 14 a cada lado.
- **Pill normal:** Placa, texto Tinta, filo. Alto 32 en escritorio, 44 junto a una primaria y 48 en el móvil.
- **Pill primary:** Placa, texto y borde en Rojo Señal. Alto 44 en emparejamiento de escritorio y 48 en el móvil.
- **Hover / Pressed:** relleno Placa Alzada y luego Filo. El borde normal pasa a Tinta Suave y luego a Tinta; el primario sigue en Rojo Señal.
- **Focus:** contorno Tinta de 1.5, separado 2.
- **Icon button:** círculo sin relleno en reposo, con el mismo relleno al pasar y pulsar. Normal en Tinta; primario con aro de 1.5 y glifo en Rojo Señal. Diámetros: 28 para descartar, 40 para transporte, 44 para navegación, 52 para reproducir y 96 para compartir en el móvil. El glifo ocupa 0.46 del diámetro.

**La Regla del Acento que Traza.** En un control, el rojo colorea borde, texto, glifo o perilla, nunca el fondo.

### Iconos
Propios y dibujados en vector. Navegación y campana van en trazo único; reproducir, anterior, siguiente y parar son sólidos.

### Cards / Containers
- **Corner Style:** placa redondeada (14).
- **Background:** Placa.
- **Shadow Strategy:** ninguna.
- **Border:** Filo, 1 px, por dentro.
- **Internal Padding:** 16.
- **Cabecera:** etiqueta numerada, punto de vivo opcional (Rojo Señal, radio 3.5) y dato alineado a la derecha opcional, con 10 de separación hasta el contenido.

### Interruptor
Fila completa pulsable con texto Control en Tinta Suave y pista delineada de 34 × 18. La perilla mide 5.5 de radio: Rojo Señal a la derecha si está encendido, Tinta Suave a la izquierda si no. El filo pasa a Tinta Suave al pasar el puntero.

### Instrumentos de puntos
- **Anillo:** 54 puntos desde arriba en sentido horario, con cifra en matriz dentro o al lado según el sitio.
- **Onda:** 34 columnas espejadas; la más alta va en Rojo Señal.
- **Radar:** anillos concéntricos alrededor del botón; un pulso rojo los recorre mientras el móvil suena.
- **Avance:** fila de puntos con cabeza roja.
- **Contorno:** rectángulo de puntos para la zona de archivos y la silueta del móvil; pasa a Rojo Señal cuando hay archivos encima.

### Puente (signature)
Tablero de 41 puntos, arco parabólico y tirantes cada 4 columnas, tenues y más pequeños. Se usa siempre entre PC y móvil, con tres estados:

- **Buscando:** se tiende y se recoge desde la PC en un ciclo de 5 s, con la punta en Rojo Señal.
- **Conectado:** se cierra una sola vez en 0.7 s con desaceleración cúbica y queda completo en Tinta.
- **Vídeo:** un paquete rojo cruza el tablero, 0.45 vueltas por segundo.

**La Regla de la Quietud Disponible.** Todo el movimiento obedece al interruptor Animaciones. Sin él, el puente queda quieto (tendido al 60 % o completo) y la información sigue visible.

## Do's and Don'ts

### Do:
- **Do** toma colores, medidas y tamaños de `theme.rs`, y dibuja las piezas con `parts.rs` y `dots.rs`.
- **Do** numera cada tarjeta nueva con índice de dos dígitos, consecutivo en su pantalla.
- **Do** representa los datos nuevos con puntos encendidos sobre puntos apagados.
- **Do** reserva Rojo Señal para activo, en vivo, atención y controles primarios.
- **Do** muestra un guion cuando falte una medida.
- **Do** mantén 48 de objetivo táctil en el móvil.
- **Do** haz que cada animación nueva respete el interruptor Animaciones.

### Don't:
- **Don't** añadas un segundo color de acento.
- **Don't** pongas Rojo Señal de fondo en tarjetas o controles; la marca de `branding/` es aparte y sí usa fondos rojos y claros.
- **Don't** uses sombras, resplandores ni rellenos en degradado en tarjetas o controles.
- **Don't** muestres cifras inventadas o decorativas fuera de la galería de desarrollo.
- **Don't** escribas texto corrido en matriz de puntos ni bajes de 5 entre puntos.
- **Don't** añadas una segunda familia tipográfica.
- **Don't** dejes que las tarjetas tapen o deformen la pantalla del móvil.
