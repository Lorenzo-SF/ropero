//! El **sistema de formato** de la salida (`ropero_core::estilo`).
//!
//! Aquí se prueba lo que no se ve a simple vista pero se nota en la
//! pantalla: el ancho visible de un texto con acentos o con color, el
//! recorte de los nombres largos y la alineación de las columnas.

use ropero_core::EngineColor;
use ropero_core::Palette;
use ropero_core::estilo::{
    Alineacion, Anchos, Celda, Columna, Seccion, Tabla, ancho_visible, pares, truncar,
};

// ─────────────────────────────────────────────────────────────────────────────
// El ancho visible
// ─────────────────────────────────────────────────────────────────────────────

/// Los códigos ANSI **no** ocupan: el color no puede desalinear nada.
#[test]
fn el_ancho_visible_ignora_los_codigos_ansi() {
    assert_eq!(ancho_visible("llama-server"), 12);
    assert_eq!(ancho_visible("\x1b[38;5;208mllama-server\x1b[39m"), 12);
    assert_eq!(ancho_visible("\x1b[38;5;208mllama\x1b[39m-server"), 12);
}

/// Las tildes y los signos valen **una** columna: no descuadran la tabla.
#[test]
fn los_acentos_y_los_signos_valen_una_columna() {
    assert_eq!(ancho_visible("mañana"), 6);
    assert_eq!(ancho_visible("áéíóú"), 5);
    assert_eq!(ancho_visible("ñ"), 1);
    assert_eq!(ancho_visible("—"), 1);
    assert_eq!(ancho_visible("·"), 1);
    assert_eq!(ancho_visible("…"), 1);
}

/// Los caracteres **anchos** (CJK, emojis) ocupan dos columnas.
#[test]
fn los_caracteres_anchos_valen_dos_columnas() {
    assert_eq!(ancho_visible("🚀"), 2);
    assert_eq!(ancho_visible("日本語"), 6);
    assert_eq!(ancho_visible("a🚀b"), 4);
}

/// Un texto con color y el mismo sin color ocupan lo mismo.
#[test]
fn el_color_no_cambia_el_ancho() {
    let con_color = "\x1b[38;5;154mturbo-llama\x1b[39m";
    assert_eq!(ancho_visible(con_color), ancho_visible("turbo-llama"));
}

// ─────────────────────────────────────────────────────────────────────────────
// El recorte
// ─────────────────────────────────────────────────────────────────────────────

/// Lo que no cabe se recorta con puntos suspensivos.
#[test]
fn lo_que_no_cabe_se_recorta_con_puntos_suspensivos() {
    let largo = "Groot-19B-A3B-261023-NVFP4.gguf";
    assert_eq!(truncar(largo, 20), "Groot-19B-A3B-26102…");
    assert_eq!(ancho_visible(&truncar(largo, 20)), 20);
}

/// Lo que sí cabe no se toca.
#[test]
fn lo_que_cabe_no_se_toca() {
    assert_eq!(truncar("groot", 10), "groot");
    assert_eq!(truncar("groot", 5), "groot");
}

// ─────────────────────────────────────────────────────────────────────────────
// La tabla
// ─────────────────────────────────────────────────────────────────────────────

/// Una tabla de dos columnas: el texto a la izquierda, los números a la
/// derecha.
#[test]
fn la_tabla_alinea_el_texto_a_la_izquierda_y_los_numeros_a_la_derecha() {
    let mut tabla = Tabla::nivel(0)
        .columna(Columna::texto("modelo"))
        .columna(Columna::numero("tamaño"));
    tabla.fila(vec![Celda::plana("groot-o"), Celda::plana("18.90 GB")]);
    tabla.fila(vec![Celda::plana("d3mini"), Celda::plana("~3.5 GB")]);
    let lineas = tabla.lineas(&Palette::plain());
    // La cabecera sigue la alineación de su columna: el título de la de
    // números cae sobre los números.
    assert_eq!(lineas[0], "modelo     tamaño");
    assert_eq!(lineas[2], "groot-o  18.90 GB");
    assert_eq!(lineas[3], "d3mini    ~3.5 GB");
}

/// La regla separa la cabecera de las filas y ocupa el ancho de la tabla.
#[test]
fn la_regla_ocupa_el_ancho_de_la_tabla() {
    let mut tabla = Tabla::nivel(0).columna(Columna::texto("modelo"));
    tabla.fila(vec![Celda::plana("groot-o")]);
    let lineas = tabla.lineas(&Palette::plain());
    assert_eq!(lineas[1], "───────");
    assert_eq!(ancho_visible(&lineas[1]), ancho_visible(&lineas[2]));
}

/// El color de la leyenda va en la celda del motor y **no** descuadra.
#[test]
fn el_color_de_la_leyenda_no_descuadra_la_tabla() {
    let mut tabla = Tabla::nivel(0)
        .columna(Columna::texto("motor"))
        .columna(Columna::numero("tamaño"));
    tabla.fila(vec![
        Celda::motor("turbo-llama", EngineColor::Amarillo),
        Celda::plana("18.90 GB"),
    ]);
    let lineas = tabla.lineas(&Palette::color());
    assert!(
        lineas[2].contains("\x1b[38;5;220m"),
        "linea={:?}",
        lineas[2]
    );
    assert_eq!(ancho_visible(&lineas[2]), "turbo-llama  18.90 GB".len());
}

/// Una columna con **máximo** recorta lo que no cabe.
#[test]
fn la_columna_con_maximo_recorta_el_artefacto() {
    let mut tabla = Tabla::nivel(0).columna(Columna::texto("artefacto").maximo(20));
    tabla.fila(vec![Celda::plana("Groot-19B-A3B-261023-NVFP4.gguf")]);
    let lineas = tabla.lineas(&Palette::plain());
    assert_eq!(lineas[2], "Groot-19B-A3B-26102…");
}

/// Las líneas **no** llevan espacios al final: rompen el diff y la copia.
#[test]
fn las_lineas_no_llevan_espacios_al_final() {
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto("modelo"))
        .columna(Columna::numero("tamaño"));
    tabla.fila(vec![Celda::plana("d3mini"), Celda::plana("~3.5 GB")]);
    for linea in tabla.lineas(&Palette::plain()) {
        assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
    }
}

/// Las tablas de una misma salida comparten las **mismas** columnas.
#[test]
fn las_tablas_comparten_anchos() {
    let mut primera = Tabla::nivel(0).columna(Columna::texto("modelo"));
    primera.fila(vec![Celda::plana("d3mini")]);
    let mut segunda = Tabla::nivel(0).columna(Columna::texto("modelo"));
    segunda.fila(vec![Celda::plana("nemotron-de-30b")]);
    let anchos = Anchos::combinado(&[&primera, &segunda]);
    let lineas = segunda.lineas_con(&anchos, &Palette::plain());
    assert_eq!(lineas[0], "modelo");
    assert_eq!(lineas[2], "nemotron-de-30b");
}

/// Una nota es una fila que ocupa toda la tabla: la razón de la fila de
/// arriba, sin romper la rejilla.
#[test]
fn una_nota_ocupa_la_tabla_entera() {
    let mut tabla = Tabla::nivel(0)
        .columna(Columna::texto("motor"))
        .columna(Columna::texto("color"));
    tabla.fila(vec![Celda::plana("fastflowlm"), Celda::plana("rojo")]);
    tabla.nota(Celda::plana("no: sólo funciona en Linux"));
    let lineas = tabla.lineas(&Palette::plain());
    assert_eq!(lineas[3], "  no: sólo funciona en Linux");
}

/// Una tabla sin filas no imprime nada: ni cabecera ni regla.
#[test]
fn una_tabla_sin_filas_no_imprime_nada() {
    let tabla = Tabla::nivel(0).columna(Columna::texto("modelo"));
    assert!(tabla.lineas(&Palette::plain()).is_empty());
}

/// La alineación por defecto de una columna de texto es a la izquierda.
#[test]
fn la_alineacion_por_defecto_es_a_la_izquierda() {
    let columna = Columna::texto("modelo");
    assert_eq!(columna.alineacion(), Alineacion::Izquierda);
    assert_eq!(Columna::numero("tamaño").alineacion(), Alineacion::Derecha);
}

// ─────────────────────────────────────────────────────────────────────────────
// La sección y los rótulos
// ─────────────────────────────────────────────────────────────────────────────

/// Una sección: el título y su cuerpo.
#[test]
fn una_seccion_tiene_titulo_y_cuerpo() {
    let mut seccion = Seccion::nueva("DEPENDENCIAS DE ROPERO");
    seccion.linea("  ok       git");
    assert_eq!(
        seccion.lineas(),
        vec![
            "DEPENDENCIAS DE ROPERO".to_string(),
            "  ok       git".to_string()
        ]
    );
}

/// Los rótulos alineados: el valor de cada par empieza en la misma columna.
#[test]
fn los_rotulos_alinean_los_valores() {
    let lineas = pares(1, &[("build", "0.1.0"), ("autor", "lorenzo")]);
    assert_eq!(lineas[0], "  build:  0.1.0");
    assert_eq!(lineas[1], "  autor:  lorenzo");
}

// ─────────────────────────────────────────────────────────────────────────────
// El ancho real, con color y con acentos
// ─────────────────────────────────────────────────────────────────────────────

/// Un acento **combinante** no ocupa columna: `e` + tilde es una `é`.
///
/// Contar `chars()` aquí duplicaría el ancho y descuadraría la fila.
#[test]
fn un_acento_combinante_no_ocupa_columna() {
    assert_eq!(ancho_visible("e\u{0301}"), 1);
    // `café` descompuesto: cuatro letras, el acento no suma columna.
    assert_eq!(ancho_visible("caf\u{0301}e"), 4);
    assert_eq!(ancho_visible("naïve"), 5);
}

/// Los guiones (el de nombre y el medio) valen **una** columna.
#[test]
fn los_guiones_valen_una_columna() {
    assert_eq!(ancho_visible("-"), 1);
    assert_eq!(ancho_visible("–"), 1);
    assert_eq!(ancho_visible("model-alpha"), 11);
    assert_eq!(ancho_visible("verde-pálido"), 12);
}

/// El recorte **cierra** el color: si no, el color se derrama sobre la
/// columna de al lado.
///
/// Lo cierra con el **mismo** reset que usa la paleta: el del primer plano
/// (`39m`). Un `0m` (reset total) es otro idioma: apagaría también el fondo
/// y los atributos que el sistema no toca.
#[test]
fn el_recorte_de_un_texto_coloreado_cierra_el_color() {
    let coloreado = "\x1b[38;5;220mturbo-llama\x1b[39m";
    let recortado = truncar(coloreado, 6);
    assert_eq!(ancho_visible(&recortado), 6, "recortado={recortado:?}");
    assert!(recortado.ends_with("\u{1b}[39m"), "recortado={recortado:?}");
    assert!(recortado.contains("turbo"), "recortado={recortado:?}");
}

/// Un rótulo con color alinea su valor en la misma columna que el resto:
/// el color **no** cuenta como ancho.
#[test]
fn un_rotulo_con_color_alinea_el_valor() {
    let lineas = pares(
        1,
        &[
            ("\x1b[38;5;208mbuild\x1b[39m", "0.1.0"),
            ("autor", "lorenzo"),
        ],
    );
    let donde = |linea: &str, valor: &str| {
        let byte = linea.find(valor).expect("debe salir el valor");
        ancho_visible(&linea[..byte])
    };
    assert_eq!(
        donde(&lineas[0], "0.1.0"),
        donde(&lineas[1], "lorenzo"),
        "lineas={lineas:?}"
    );
    for linea in &lineas {
        assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
    }
}

/// La regla y la cabecera miden lo mismo que la fila más ancha: la rejilla
/// **cuadra** cuando hay color en una celda.
#[test]
fn la_regla_cuadra_con_la_fila_coloreada() {
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto("motor"))
        .columna(Columna::numero("tamaño"));
    tabla.fila(vec![
        Celda::motor("turbo-llama", EngineColor::Amarillo),
        Celda::plana("~18.90 GB"),
    ]);
    tabla.fila(vec![Celda::plana("groot"), Celda::plana("~3.50 GB")]);
    let lineas = tabla.lineas(&Palette::color());
    let regla = ancho_visible(&lineas[1]);
    assert_eq!(regla, ancho_visible(&lineas[0]), "lineas={lineas:?}");
    for linea in &lineas[2..] {
        assert_eq!(regla, ancho_visible(linea), "linea={linea:?}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Los bordes: el título, los controles y los emojis
// ─────────────────────────────────────────────────────────────────────────────

/// El título de una columna **respeta su máximo**: no desborda la regla.
///
/// Una cabecera más ancha que su columna empuja la regla y descuadra la
/// tabla entera: el título se recorta como los datos.
#[test]
fn el_titulo_de_la_columna_no_desborda_la_regla() {
    let mut tabla = Tabla::nivel(0)
        .columna(Columna::texto("un título de columna muy largo").maximo(10))
        .columna(Columna::texto("modelo"));
    tabla.fila(vec![Celda::plana("groot-o"), Celda::plana("groot")]);
    let lineas = tabla.lineas(&Palette::plain());
    // El título se recorta, como cualquier dato que no cabe.
    assert!(lineas[0].contains('…'), "cabecera={:?}", lineas[0]);
    // Y la cabecera no es más ancha que la regla.
    assert_eq!(
        ancho_visible(&lineas[0]),
        ancho_visible(&lineas[1]),
        "la cabecera desborda la regla: {lineas:?}"
    );
}

/// Una celda **no** se parte en dos líneas (doc CLI v2 §2.1).
///
/// El tabulador y el salto de línea se sustituyen por un espacio: lo que
/// no cabe en la rejilla se recorta, y lo que la rompe se sustituye.
#[test]
fn la_celda_no_lleva_saltos_ni_tabuladores() {
    let mut tabla = Tabla::nivel(0).columna(Columna::texto("modelo"));
    tabla.fila(vec![Celda::plana("groot\to\nd3mini")]);
    let lineas = tabla.lineas(&Palette::plain());
    assert_eq!(lineas.len(), 3, "la celda parte la tabla: {lineas:?}");
    assert!(!lineas[2].contains('\t'), "linea={:?}", lineas[2]);
    assert!(!lineas[2].contains('\n'), "linea={:?}", lineas[2]);
    assert_eq!(lineas[2], "groot o d3mini");
}

/// Los emojis miden lo que miden en la pantalla.
#[test]
fn los_emojis_miden_lo_que_miden() {
    // El signo de advertencia es de los anchos: ocupa dos columnas.
    assert_eq!(ancho_visible("⚠"), 2);
    assert_eq!(ancho_visible("⚠️"), 2);
    // Un modificador de tono de piel no ocupa columna: va pegado al emoji.
    assert_eq!(ancho_visible("👍🏽"), 2);
    assert_eq!(ancho_visible("👋🏻"), 2);
    // Y el emoji con su selector de variación sigue midiendo dos.
    assert_eq!(ancho_visible("🚀"), 2);
}
