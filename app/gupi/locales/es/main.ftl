app-title = Gupi
menu-settings = Configuración
menu-show-main = Ventana principal
menu-quit = Salir de Gupi
startup-welcome = Te damos la bienvenida a Gupi
temporary-setup-required = Completa la configuración en Ajustes para iniciar una conversación temporal.
startup-checking = Comprobando…
startup-quitting = Terminando las operaciones…
settings-pi-command = Ejecutable de Pi
settings-path-help = Déjalo vacío para buscar pi automáticamente o introduce la ruta absoluta del ejecutable, sin argumentos.
settings-theme = Apariencia
settings-language = Idioma
settings-save-pi = Guardar la ruta de Pi
settings-reload = Volver a cargar desde el disco
settings-write-current = Guardar en disco los ajustes aplicados (conservar el borrador)
theme-system = Sistema
theme-light = Claro
theme-dark = Oscuro
language-system = Sistema
language-english = Inglés
language-chinese = 简体中文
recovery-config-title = La configuración requiere atención
recovery-pi-title = Configurar Pi
recovery-confirm = ¿Continuar? Volver a cargar descartará el borrador solo si se completa correctamente. Antes de restablecer, se hará una copia de seguridad del archivo actual.
recovery-backup = Copia de seguridad
error-config-read = No se pudo leer la configuración. Comprueba la ubicación y los permisos y vuelve a intentarlo.
error-config-parse = La configuración no es válida. Corrige el archivo y vuelve a cargarlo, o haz una copia de seguridad y restablece la configuración.
error-config-validation = Introduce el nombre de un ejecutable o una ruta absoluta a un ejecutable, sin argumentos.
error-config-write = No se pudo guardar. Se conservaron el borrador y los ajustes aplicados. Comprueba el archivo de configuración e inténtalo de nuevo.
error-pi-probe = No se pudo verificar Pi. Comprueba la ruta del ejecutable y su entorno de Node y vuelve a intentarlo.
action-check-pi = Volver a comprobar Pi
action-reset = Hacer una copia de seguridad y restablecer
action-confirm = Confirmar
action-cancel = Cancelar
home-ready = Pi está disponible
home-description = Esta versión ofrece la configuración del entorno y los ajustes de escritorio. La compatibilidad con conversaciones llegará en la siguiente etapa.
action-locate = Mostrar la carpeta de configuración
error-pi-timeout = Pi no terminó la comprobación de versión en 15 segundos.
error-pi-output = Pi devolvió más contenido del permitido para una comprobación de versión.
error-pi-version = Pi terminó con un error o no devolvió una versión válida.
error-log = No se puede escribir el registro. Comprueba la carpeta de registros y sus permisos.

setup-tagline = Tu espacio de escritorio nativo para Pi
setup-intro = Una configuración rápida para empezar.
setup-start = Empezar
setup-language-title = A tu gusto
setup-search-language = Buscar idiomas…
setup-system-language = Idioma del sistema: { $language }
setup-appearance-title = Personaliza la apariencia
setup-color-mode = Modo de color
setup-system-accent = Acento del sistema
setup-pi-title = Conectar Pi
setup-pi-step = Configuración de Pi
setup-pi-help = Comprueba una instalación local de Pi ahora o salta esta página y configúrala más tarde.
setup-check-pi = Comprobar Pi
setup-back = Atrás
setup-next = Continuar
setup-finish = Finalizar configuración
setup-saving = Guardando…
light-themes = Temas claros
dark-themes = Temas oscuros

# Espacio de conversaciones
conversation-new = Nueva conversación
conversation-sidebar = Barra lateral de conversaciones
conversation-history = Historial de conversaciones
conversation-restoring = Restaurando borradores…
conversation-discovering = Buscando conversaciones… { $count } archivos
conversation-read-progress = Leídos { $completed } de { $total } archivos
conversation-refresh-progress = Actualizando { $completed } de { $total } archivos
conversation-project = Elegir carpeta de trabajo
conversation-untitled = Conversación sin título
conversation-idle = Inactiva
conversation-loading = Abriendo sesión
conversation-running = En ejecución
conversation-failed = Error que requiere atención
conversation-waiting = Esperando una respuesta
conversation-search = Buscar conversaciones
conversation-refresh = Actualizar conversaciones
conversation-refresh-current = Actualizar la conversación actual
conversation-actions = Acciones de sesión
conversation-show-sidebar = Mostrar barra lateral de conversaciones
conversation-hide-sidebar = Ocultar barra lateral de conversaciones
conversation-scan-failed = No se pudo cargar el catálogo de conversaciones
conversation-search-placeholder = Buscar nombres, mensajes o rutas de proyectos
conversation-search-empty = No hay conversaciones que coincidan
conversation-rename = Cambiar nombre…
conversation-delete = Mover a la papelera
conversation-delete-failed = No se pudo eliminar la conversación: { $error }
conversation-copy-path = Copiar ruta
conversation-clone = Duplicar conversación
conversation-export = Exportar conversación como HTML…
conversation-exported = Conversación exportada a { $path }
conversation-stop = Detener la generación
conversation-close-run = Cerrar ejecución
conversation-show-less = Mostrar menos
conversation-show-more = Mostrar más…
conversation-fork = Crear una bifurcación desde aquí
conversation-preview = Vista previa de otra rama
conversation-return-current = Volver a la rama actual
conversation-welcome = Iniciar una conversación
conversation-welcome-project = ¿Qué quieres hacer en { $project }?
conversation-empty-history = Aún no hay mensajes en esta conversación.
conversation-bottom = Ir al final
conversation-copy = Copiar
conversation-compaction = Resumen de compactación del contexto
conversation-branch-summary = Resumen de la rama
conversation-working = Procesando…
conversation-process = Ver proceso
conversation-details = Mostrar u ocultar detalles
tool-detail-offset = Línea inicial:
tool-detail-line-limit = Límite de líneas solicitado:
tool-detail-timeout = Tiempo de espera (segundos):
tool-detail-output = Salida
tool-detail-input = Entrada
tool-detail-additional = Información adicional
tool-detail-error = Error
tool-detail-glob = Patrón de archivos:
tool-detail-ignore-case = Ignorar mayúsculas y minúsculas:
tool-detail-literal = Búsqueda literal:
tool-detail-context = Líneas de contexto:
tool-detail-result-limit = Límite de resultados solicitado:
tool-detail-old-text = Texto original solicitado
tool-detail-new-text = Texto de reemplazo solicitado
tool-detail-truncated = Pi truncó esta salida; el contenido mostrado no es el resultado completo.
tool-detail-lines-truncated = Pi acortó algunas líneas de resultados.
tool-detail-match-limit = Se alcanzó el límite de coincidencias:
tool-detail-results-limited = Se alcanzó el límite de resultados:
tool-detail-entries-limited = Se alcanzó el límite de entradas del directorio:
tool-detail-full-output = Archivo con la salida completa:
tool-detail-image-unavailable = No se puede mostrar esta imagen.
conversation-role-user = Mensaje del usuario
conversation-role-assistant = Mensaje del asistente
conversation-role-tool = Resultado de la herramienta
conversation-event = Evento de sesión
conversation-close-history = Cerrar el historial de conversaciones
conversation-source = Mostrar el archivo de origen de la sesión
conversation-reconnect = Volver a conectar
conversation-save-error = No se pudo guardar el borrador
conversation-interrupted = Ejecución detenida; se conserva el contenido existente.
conversation-no = No
conversation-submit = Enviar
conversation-input = Escribe un mensaje
conversation-model = Modelo
conversation-load-options = Conecta con Pi para cargar modelos
conversation-thinking = Nivel de razonamiento
conversation-unknown = No disponible
conversation-context = Contexto de la rama de ejecución actual
conversation-auto-compaction = Compactación automática
conversation-on = Activado
conversation-off = Desactivado
conversation-tokens = Total de entrada y salida
conversation-cache = Total leído y escrito en caché
conversation-cache-hit = Tasa de aciertos de caché reciente
conversation-cost = Coste indicado por Pi
conversation-statistics = Estadísticas de uso
conversation-send = Enviar (Intro); Alt+Intro añade una tarea de seguimiento
conversation-sending = Enviando…
conversation-graph-current = Actual
conversation-graph-preview = Vista previa
conversation-graph-left = Mostrar las ramas de la izquierda (o desplázate horizontalmente)
conversation-graph-right = Mostrar las ramas de la derecha (o desplázate horizontalmente)
conversation-graph-reveal = Mostrar la selección
conversation-graph-column = Grafo
conversation-graph-message = Mensaje
conversation-catalog-empty = Aún no hay conversaciones

conversation-thinking-content = Razonamiento
conversation-thinking-running = Pensando…
conversation-tool-group = Llamadas a herramientas ({ $count })
conversation-tool-group-read = Lecturas de archivos ({ $count })
conversation-tool-group-bash = Comandos ({ $count })
conversation-tool-group-search = Búsquedas ({ $count })
conversation-tool-group-edit = Cambios en archivos ({ $count })
conversation-tool-line = { $action } { $summary }
conversation-tool-action-read =
    { $state ->
        [running] Leyendo
        [complete] Leído
        [failed] No se pudo leer
        *[unfinished] Lectura sin terminar
    }
conversation-tool-action-write =
    { $state ->
        [running] Escribiendo
        [complete] Escrito
        [failed] No se pudo escribir
        *[unfinished] Escritura sin terminar
    }
conversation-tool-action-edit =
    { $state ->
        [running] Editando
        [complete] Editado
        [failed] No se pudo editar
        *[unfinished] Edición sin terminar
    }
conversation-tool-action-bash =
    { $state ->
        [running] Ejecutando
        [complete] Ejecutado
        [failed] Falló el comando
        *[unfinished] Comando sin terminar
    }
conversation-tool-action-search =
    { $state ->
        [running] Buscando
        [complete] Búsqueda completada
        [failed] Falló la búsqueda
        *[unfinished] Búsqueda sin terminar
    }
conversation-tool-action-other =
    { $state ->
        [running] Llamando a { $name }
        [complete] Se llamó a { $name }
        [failed] No se pudo llamar a { $name }
        *[unfinished] Llamada sin terminar
    }

conversation-processed = Procesado durante { $duration }
conversation-processed-failed = Falló después de { $duration }
conversation-processed-stopped = Detenido después de { $duration }
conversation-copied = Copiado
conversation-copy-failed = No se pudo copiar. Inténtalo de nuevo.
conversation-usage-title = Uso de la solicitud
conversation-usage-model = Modelo
conversation-usage-provider = Proveedor
conversation-usage-input = Tokens de entrada
conversation-usage-output = Tokens de salida
conversation-usage-cache-read = Tokens leídos de la caché
conversation-usage-cache-write = Tokens escritos en la caché
conversation-usage-total = Tokens totales
conversation-usage-cost = Coste

composer-context-used = Tokens usados
composer-context-limit = Capacidad del contexto
composer-context-percent = Contexto usado
composer-token-input = Tokens de entrada totales
composer-token-output = Tokens de salida totales
composer-token-cache-read = Tokens leídos de la caché totales
composer-token-cache-write = Tokens escritos en la caché totales
composer-token-usage = Uso de tokens de la sesión

conversation-model-search = Buscar modelos…
conversation-model-empty = No hay modelos que coincidan
conversation-thinking-off = Desactivado
conversation-thinking-minimal = Mínimo
conversation-thinking-low = Bajo
conversation-thinking-medium = Medio
conversation-thinking-high = Alto
conversation-thinking-xhigh = Muy alto
conversation-thinking-max = Máximo

conversation-model-reasoning = Razonamiento
conversation-model-vision = Visión

conversation-history-reply = Respuesta del asistente
conversation-history-brief = Breve: mensajes del usuario y respuestas finales
conversation-history-detailed = Detallado: mensajes, herramientas y resúmenes
conversation-history-progress = Progreso del asistente
conversation-history-thinking = Razonamiento
conversation-history-calls = Llamadas a herramientas
conversation-history-failed = La ejecución falló
conversation-history-stopped = Interrumpida
conversation-history-empty = Respuesta sin texto
composer-model-thinking = Modelo y razonamiento
composer-model-refresh = Actualizar modelos y niveles de razonamiento
composer-thinking-unavailable = No compatible
composer-model-loading = Cargando modelos…
composer-thinking-loading = Cargando niveles de razonamiento…
composer-model-confirming = Confirmando los ajustes del modelo…
composer-stats-loading = Cargando uso…
composer-stats-retry = Volver a cargar el uso
conversation-checking-file = Comprobando el archivo de conversación…
conversation-connecting = Conectando con Pi…
conversation-history-loading = Cargando el historial de conversaciones…
conversation-history-refreshing = Actualizando el historial de conversaciones…
conversation-fork-options-loading = Cargando opciones de bifurcación…
conversation-fork-options-retry = Volver a cargar las opciones de bifurcación
history-canvas-zoom-in = Acercar
history-canvas-zoom-out = Alejar
history-canvas-fit = Ajustar el árbol (0)
history-canvas-current = Ubicar el nodo de ejecución
history-canvas-expand = Expandir { $count } nodos
history-canvas-collapse = Contraer { $count } nodos de proceso y conservar los marcadores de navegación
history-canvas-empty = No hay nodos del historial para mostrar
history-canvas-help = Árbol de conversación: desplázate o arrastra para moverte, pellizca o usa más/menos para cambiar el zoom, usa las flechas para seleccionar, Intro para obtener una vista previa y E para expandir el siguiente segmento

history-view-tree = Árbol
history-view-list = Lista
history-level-brief = Breve
history-level-detailed = Detallado
history-level-all = Todo
history-level-all-description = Todo: incluye ajustes, etiquetas y registros personalizados
history-scope-all = Todas las ramas
history-scope-branch = Solo esta rama
history-model-change = Cambio de modelo
history-thinking-change = Cambio del nivel de razonamiento
history-session-info = Cambio de información de sesión
history-label-change = Cambio de etiqueta
history-custom-record = Registro personalizado


history-content = Contenido
history-range = Alcance

command-palette = Paleta de comandos
command-focus-input = Enfocar el campo de entrada de la conversación
command-model = Seleccionar modelo y nivel de razonamiento…
command-copy-last-answer = Copiar la última respuesta
command-compact = Compactar contexto
conversation-compacting = Compactando el contexto…
command-history-description = Ver el historial o elegir un mensaje del usuario para crear una bifurcación
command-show-history = Mostrar historial
command-hide-history = Ocultar historial
command-current-session = Sesión actual:
command-unavailable = No disponible en el estado actual
command-target-changed = Esta conversación ya no está disponible. Selecciona otra.
command-scanning = Cargando conversaciones…
command-loading = Cargando comandos de Pi…
command-no-matches = No hay comandos de Pi que coincidan
conversation-reconnect-unconfirmed = No se pudo confirmar que el proceso de Pi haya terminado. Reinicia Gupi antes de volver a conectar.
action-retry = Reintentar

command-search-placeholder = Buscar acciones o introducir un comando
command-empty = No hay comandos de Pi disponibles en esta sesión

command-group-app = Aplicación
command-group-extensions = Comandos de extensiones
command-group-skills = Habilidades
command-group-prompts = Plantillas de instrucciones
command-scope-user = Personal
command-scope-project = Proyecto
command-scope-temporary = Temporal
command-connection-unavailable = Pi no está listo
command-dismiss = Cerrar
command-complete = Completar
command-send-text = Enviar
command-execute = Ejecutar

command-scope-current = Sesión actual

conversation-working-duration = Procesando durante { $duration }
conversation-tool-group-skill = Lecturas de habilidades ({ $count })
conversation-shell-line = { $shell } · { $action } { $summary }
conversation-tool-action-skill =
    { $state ->
        [running] Leyendo la habilidad
        [complete] Habilidad leída
        [failed] No se pudo leer la habilidad
       *[unfinished] Lectura de habilidad sin terminar
    }

# Ajustes unificados
settings-page-general = General
settings-page-pi = Pi
settings-page-keys = Atajos de teclado
settings-page-plugins = Complementos
settings-page-skills = Habilidades
settings-page-prompts = Instrucciones
settings-page-about = Acerca de
settings-key-reset = Restaurar el valor predeterminado
settings-key-reset-all = Restaurar todos los valores predeterminados
settings-key-reset-all-confirm = ¿Restaurar los atajos predeterminados? Se borrarán todos los atajos personalizados.
settings-key-clear = Borrar atajo
settings-key-cancel = Descartar cambios
settings-key-unbound = Sin atajo asignado
settings-key-record = Grabar
settings-key-recording = Pulsa un atajo…
settings-key-conflict = Atajo no válido o en conflicto
settings-key-invalid = Introduce un atajo válido o deja el campo vacío para quitarlo
settings-resource-refresh = Actualizar recursos
settings-resource-reload-help = Solo recursos personales. Vuelve a cargar las sesiones existentes manualmente para aplicar los cambios.
settings-resource-working = Procesando…
settings-resource-pi-required = Guarda y comprueba primero un ejecutable de Pi válido en Ajustes de Pi.
settings-resource-invalid-name = Los nombres pueden contener letras, números, guiones y guiones bajos.
settings-resource-register = Añadir localmente
settings-resource-create = Crear
settings-resource-name-help = Nombre del recurso nuevo: letras, números, guiones o guiones bajos.
settings-resource-empty = No hay recursos que coincidan.
settings-resource-delete = Mover a la papelera
settings-resource-save = Guardar texto
shortcut-save-task = Guardar
settings-resource-loading = Leyendo texto…
settings-resource-confirm-remove = ¿Quitar este elemento? Al eliminar un archivo, solo se mueve a la papelera el archivo seleccionado; se conserva su carpeta.
settings-package-install = Instalar paquete
settings-package-update = Actualizar
settings-package-remove = Quitar
settings-package-source-help = Origen del paquete: npm:nombre, una URL de Git o una ruta local absoluta.
settings-package-extensions = Extensiones { $count }
settings-package-skills = Habilidades { $count }
settings-package-prompts = Plantillas { $count }
settings-package-themes = Temas { $count }
settings-skill-search = Buscar habilidades por nombre, descripción, ruta u origen
settings-skill-collapse = Contraer
settings-resource-edit = Editar
settings-about-gupi = Versión
settings-about-pi = Versión
settings-about-path = Ruta resuelta
settings-about-status = Estado de la comprobación
settings-about-unavailable = Todavía no está disponible
settings-editor-discard = ¿Descartar los cambios de texto sin guardar?
settings-package-confirm-remove = ¿Quitar este paquete de los ajustes personales de Pi? Se conservan las carpetas de origen de los paquetes locales.
settings-package-heading = Paquetes instalados
settings-extension-heading = Extensiones independientes
settings-template-heading = Plantillas de comandos
settings-system-heading = Instrucciones del sistema
settings-config-heading = Archivo de configuración
settings-config-open = Abrir archivo de configuración
settings-mode-help = Sigue el sistema para cambiar automáticamente entre temas claros y oscuros.
settings-light-help = Se usa cuando está activo el modo claro.
settings-dark-help = Se usa cuando está activo el modo oscuro.
settings-pi-unsaved = Ruta sin guardar. La comprobación valida la entrada actual sin guardarla.
settings-pi-saved = La ruta actual está guardada. Vuelve a comprobarla para verificar que Pi está disponible.
settings-resource-location = Mostrar ubicación
settings-package-source = Origen del paquete
settings-package-empty = No hay paquetes personales de Pi instalados.
settings-resource-name = Nombre
settings-resource-none = Todavía no hay recursos personales de este tipo.
settings-resource-view = Ver
settings-resource-readonly-badge = Solo lectura
settings-source-personal = Personal
settings-source-external = Externo
settings-system-replace = Reemplazar las instrucciones del sistema predeterminadas
settings-system-replace-help = Reemplaza por completo las instrucciones del sistema predeterminadas de Pi.
settings-system-append = Añadir instrucciones personales
settings-system-append-help = Conserva las instrucciones del sistema predeterminadas y añade estas instrucciones.
settings-resource-success = Se completó la operación en «{ $target }». Vuelve a cargar las sesiones existentes manualmente.
settings-resource-failed = La operación en «{ $target }» falló.
settings-about-help = Muestra el resultado más reciente de la comprobación de la ruta guardada. Abre los ajustes de Pi para volver a comprobarla.
settings-key-reveal-session = Mostrar archivo de sesión

settings-key-group-app = Aplicación
settings-key-group-conversation = Conversación
settings-key-group-files = Archivos de sesión

settings-package-kind-extensions = Extensiones
settings-package-kind-skills = Habilidades
settings-package-kind-themes = Temas
settings-package-kind-prompts = Plantillas
settings-package-expand = Expandir el contenido del paquete
settings-package-collapse = Contraer el contenido del paquete

settings-template-search = Buscar plantillas por nombre, descripción u origen
settings-template-collapse = Contraer vista previa

temporary-title = Conversaciones temporales
attachment-add = Adjuntar archivos
attachment-remove = Quitar archivo adjunto
attachment-file = Archivo
attachment-clipboard = Imagen del portapapeles
image-preview-close = Cerrar vista previa
image-preview-open = Abrir vista previa de la imagen
image-preview-zoom-in = Acercar
image-preview-zoom-out = Alejar
shortcut-global = Atajos globales
shortcut-launcher = Ventana temporal
shortcut-add = Añadir tarea de plantilla
shortcut-edit = Editar tarea de plantilla
shortcut-delete = Eliminar tarea de plantilla
shortcut-name = Nombre
shortcut-template = Plantilla de comando
shortcut-source = Origen de entrada
shortcut-selection = Texto seleccionado
shortcut-clipboard = Portapapeles
shortcut-fallback = Texto seleccionado; si no, portapapeles
shortcut-model = Modelo
shortcut-thinking = Nivel de razonamiento
shortcut-default-model = Modelo predeterminado de Pi
shortcut-default-thinking = Nivel de razonamiento predeterminado de Pi
shortcut-personal = Personal
shortcut-template-required = Selecciona una plantilla de comando.
shortcut-reload-options = Volver a cargar opciones
temporary-clean-released = Espacios de trabajo liberados
temporary-clean = Limpiar
temporary-clean-help = Mueve a la papelera los espacios de trabajo temporales liberados. Las conversaciones activas se conservan.
temporary-cleaned = Espacios de trabajo movidos a la papelera
shortcut-tasks = Tareas de plantilla

shortcut-select-template = Elegir una plantilla de instrucciones

temporary-search-placeholder = Buscar conversaciones temporales
temporary-search-empty = No hay conversaciones temporales que coincidan
temporary-toggle-input = Cambiar campo de entrada
temporary-hide = Ocultar
session-search-open = Abrir conversación

temporary-actions = Acciones
temporary-paste-answer = Pegar la última respuesta
temporary-reveal-workspace = Mostrar carpeta de trabajo
temporary-search-actions = Buscar acciones…
temporary-switch-session = Cambiar de conversación temporal
temporary-paste-failed = Se copió la respuesta. No se pudo pegar automáticamente. Comprueba el permiso de Accesibilidad y la aplicación de destino, o pégala manualmente.
temporary-session-1 = Cambiar a la conversación temporal 1
temporary-session-2 = Cambiar a la conversación temporal 2
temporary-session-3 = Cambiar a la conversación temporal 3
temporary-session-4 = Cambiar a la conversación temporal 4
temporary-session-5 = Cambiar a la conversación temporal 5
temporary-session-6 = Cambiar a la conversación temporal 6
temporary-session-7 = Cambiar a la conversación temporal 7
temporary-session-8 = Cambiar a la conversación temporal 8
temporary-session-9 = Cambiar a la última conversación temporal
temporary-send = Enviar
temporary-trash = Mover la conversación temporal a la papelera
settings-key-stop-or-hide = Detener la generación u ocultar la ventana temporal

conversation-queue-title = En cola ({ $count })
conversation-queue-steer = Dirigir el turno actual
conversation-queue-follow-up = Tarea de seguimiento
conversation-queue-restore = Devolver todo el texto en cola al borrador
conversation-queue-clear = Borrar todos los mensajes en cola
conversation-queue-no-text = Mensaje sin texto
conversation-queue-unavailable = El contenido de la cola aún no está disponible
conversation-queue-text-only = Al devolver la cola solo se restaura el texto; no se pueden restaurar las imágenes en cola.

conversation-retry-countdown = Reintento { $attempt }/{ $total } en unos { $seconds } s
conversation-retry-waiting = Reintento { $attempt }/{ $total }: esperando a Pi
conversation-summary-retry-countdown = Reintento del resumen { $attempt }/{ $total } en unos { $seconds } s
conversation-summary-retry-waiting = Reintento del resumen { $attempt }/{ $total }: esperando a Pi

message-details-open = Ver detalles…
message-details-copy-all = Copiar todo
tool-detail-command = Comando
tool-detail-content = Contenido
tool-detail-changes = Cambios

settings-notifications = Notificaciones
settings-notification-help = Las notificaciones del sistema muestran una descripción general. Ábrela para volver a la conversación. El recuento de elementos sin leer se borra cuando la conversación está abierta en primer plano.
settings-notification-waiting = Avisar cuando se necesite una respuesta en segundo plano
settings-notification-failures = Avisar cuando falle una tarea en segundo plano
settings-notification-plugins = Enviar recordatorios de complementos como notificaciones del sistema en segundo plano
settings-notification-attention = Solicitar atención cuando se necesite una respuesta en segundo plano
settings-notification-completion = Notificaciones de respuesta completada
notification-off = Desactivadas
notification-background = Solo en segundo plano
notification-always = Siempre
notification-waiting = Esperando tu respuesta
notification-completed = Tu respuesta está lista
notification-failed = Una tarea requiere atención tras un error
notification-plugin = Recordatorios de complementos
notification-unread = Conversaciones sin leer
notification-running = En ejecución
notification-clear = Borrar recordatorios
notification-session-notices = Avisos de conversación

settings-icon-theme = Icono de la aplicación
settings-icon-theme-help = Cambia el logotipo dentro de la aplicación y el icono del Dock de macOS mientras Gupi está en ejecución. El Finder y la barra de tareas de Windows conservan el icono del paquete.
icon-theme-classic = Clásico
icon-theme-classic-gradient = Clásico con degradado
icon-theme-color = Pi en color
icon-theme-color-gradient = Pi con degradado
icon-theme-pride = Arcoíris
icon-theme-ukraine = Ucrania
icon-theme-ukraine-gradient = Ucrania con degradado

language-traditional-chinese = Chino tradicional
language-japanese = Japonés
language-korean = Coreano
language-german = Alemán
language-french = Francés
language-spanish = Español
language-portuguese-brazil = Portugués (Brasil)
menu-about = Acerca de Gupi
menu-conversation = Conversación
menu-edit = Edición
menu-view = Visualización
menu-window = Ventana
menu-help = Ayuda
menu-services = Servicios
menu-hide = Ocultar Gupi
menu-hide-others = Ocultar otras aplicaciones
menu-show-all = Mostrar todo
menu-undo = Deshacer
menu-redo = Rehacer
menu-cut = Cortar
menu-copy = Copiar
menu-paste = Pegar
menu-select-all = Seleccionar todo
menu-minimize = Minimizar
menu-zoom = Zoom
menu-fullscreen = Alternar pantalla completa
menu-docs = Guía de usuario de Gupi
menu-pi-docs = Documentación de Pi
menu-report-issue = Informar de un problema
menu-logs = Mostrar registros
menu-copy-diagnostics = Copiar diagnóstico
settings-permissions = Permisos del sistema
settings-accessibility-help = Seleccionar texto y pegarlo en otra aplicación puede requerir acceso a Accesibilidad. Activa Gupi en Ajustes del Sistema y vuelve a intentarlo. Si falla el pegado, podrás copiar la respuesta.
settings-accessibility-open = Ajustes de Accesibilidad
settings-notification-permission-help = Si no aparecen las alertas o la insignia del Dock, permite las notificaciones y las insignias de Gupi en Ajustes del Sistema. El permiso se solicita la primera vez que se usa la función.
settings-notification-permission-open = Ajustes de notificaciones
settings-native-language-help = El idioma de la aplicación cambia inmediatamente. Reinicia Gupi para aplicar el cambio a los diálogos del sistema de macOS. Los diálogos del sistema de Windows siguen el idioma para mostrar de Windows.
settings-diagnostics-help = Incluye la versión de la aplicación, la plataforma, el comando de Pi y las ubicaciones de los archivos de configuración y registro. No incluye conversaciones, credenciales ni variables de entorno.

setup-preferences-title = Idioma y apariencia
setup-preferences-step = Idioma y apariencia
setup-appearance-options = Elegir un tema y un icono
setup-pi-install-guide = Instalar Pi
setup-pi-model-guide = Conectar un modelo
setup-desktop-title = Atajos y notificaciones
setup-desktop-step = Atajos y notificaciones
setup-desktop-help = Puedes cambiar estas opciones más tarde en Ajustes.
setup-page-skip = Saltar esta página
setup-skip-all = Usar los valores predeterminados
setup-pi-later = Configurar Pi más tarde
action-open = Abrir
