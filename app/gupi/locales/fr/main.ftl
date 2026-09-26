app-title = Gupi
menu-settings = Réglages
menu-show-main = Fenêtre principale
menu-quit = Quitter Gupi
startup-welcome = Bienvenue dans Gupi
temporary-setup-required = Terminez la configuration dans Réglages pour démarrer une conversation temporaire.
startup-checking = Vérification…
startup-quitting = Finalisation des opérations…
settings-pi-command = Exécutable Pi
settings-path-help = Laissez vide pour rechercher pi automatiquement, ou indiquez un chemin absolu vers l’exécutable sans arguments.
settings-theme = Apparence
settings-language = Langue
settings-save-pi = Enregistrer le chemin de Pi
settings-reload = Recharger depuis le disque
settings-write-current = Écrire les réglages appliqués sur le disque (conserver le brouillon)
theme-system = Système
theme-light = Clair
theme-dark = Sombre
language-system = Système
language-english = Anglais
language-chinese = 简体中文
recovery-config-title = La configuration nécessite votre attention
recovery-startup-title = Gupi n’a pas pu démarrer
error-instance-startup = Vérifiez les autorisations du dossier de configuration ou fermez les autres copies de Gupi en cours d’exécution, puis réessayez.
recovery-pi-title = Configurer Pi
recovery-confirm = Continuer ? Le rechargement n’abandonne le brouillon qu’en cas de réussite. Une sauvegarde est créée avant la réinitialisation.
recovery-backup = Sauvegarde
error-config-read = Impossible de lire la configuration. Vérifiez son emplacement et ses autorisations, puis rechargez-la.
error-config-parse = La configuration est invalide. Rechargez un fichier corrigé ou sauvegardez puis réinitialisez le fichier actuel.
error-config-validation = Saisissez un nom d’exécutable ou un chemin absolu vers l’exécutable, sans arguments.
error-config-write = Échec de l’enregistrement. Le brouillon et les réglages appliqués sont conservés. Vérifiez le fichier de configuration puis réessayez.
error-pi-probe = Impossible de vérifier Pi. Vérifiez le chemin de l’exécutable et son environnement Node, puis réessayez.
action-check-pi = Vérifier Pi à nouveau
action-reset = Sauvegarder et réinitialiser
action-confirm = Confirmer
action-cancel = Annuler
home-ready = Pi est disponible
home-description = Cette version propose la configuration de l’environnement et les préférences du bureau. La prise en charge des conversations est prévue pour la prochaine étape.
action-locate = Afficher le dossier de configuration
error-pi-timeout = Pi n’a pas terminé la vérification de version en 15 secondes.
error-pi-output = Pi a renvoyé plus de sortie que la vérification de version ne l’autorise.
error-pi-version = Pi s’est arrêté avec une erreur ou n’a pas renvoyé de version valide.
error-log = La journalisation dans un fichier est indisponible. Vérifiez le dossier des journaux et ses autorisations.

setup-tagline = Votre espace de bureau natif pour Pi
setup-intro = Une configuration rapide pour commencer.
setup-start = Commencer
setup-language-title = Installez-vous confortablement
setup-search-language = Rechercher une langue…
setup-system-language = Langue du système : { $language }
setup-appearance-title = Personnalisez votre espace
setup-color-mode = Mode de couleur
setup-system-accent = Couleur d’accentuation du système
setup-pi-title = Connecter Pi
setup-pi-step = Configuration de Pi
setup-pi-help = Vérifiez une installation locale de Pi maintenant, ou ignorez cette page pour la configurer plus tard.
setup-check-pi = Vérifier Pi
setup-back = Retour
setup-next = Continuer
setup-finish = Terminer la configuration
setup-saving = Enregistrement…
light-themes = Thèmes clairs
dark-themes = Thèmes sombres

# Conversation workspace
conversation-new = Nouvelle conversation
conversation-sidebar = Barre latérale des conversations
conversation-history = Historique des conversations
conversation-restoring = Restauration des brouillons…
conversation-discovering = Recherche des conversations… { $count } fichiers
conversation-read-progress = Lecture de { $completed } / { $total } fichiers
conversation-refresh-progress = Actualisation de { $completed } / { $total } fichiers
conversation-project = Choisir le dossier de travail
conversation-untitled = Conversation sans titre
conversation-idle = Inactive
conversation-loading = Ouverture de la session
conversation-running = En cours
conversation-failed = Erreur nécessitant votre attention
conversation-waiting = En attente de saisie
conversation-search = Rechercher des conversations
conversation-refresh = Actualiser les sessions
conversation-refresh-current = Actualiser la session actuelle
conversation-actions = Actions de session
conversation-show-sidebar = Afficher la barre latérale des sessions
conversation-hide-sidebar = Masquer la barre latérale des sessions
conversation-scan-failed = Impossible de charger le catalogue des conversations
conversation-search-placeholder = Rechercher des noms, messages ou chemins de projet
conversation-search-empty = Aucune conversation correspondante
conversation-rename = Renommer…
conversation-delete = Déplacer vers la corbeille
conversation-delete-failed = Impossible de supprimer la conversation : { $error }
conversation-copy-path = Copier le chemin
conversation-clone = Dupliquer la conversation
conversation-export = Exporter la conversation au format HTML…
conversation-exported = Conversation exportée vers { $path }
conversation-stop = Arrêter la génération
conversation-close-run = Fermer le processus
conversation-show-less = Afficher moins
conversation-show-more = Afficher plus…
conversation-fork = Créer une branche à partir d’ici
conversation-preview = Aperçu d’une autre branche
conversation-return-current = Revenir à la branche actuelle
conversation-welcome = Démarrer une conversation
conversation-welcome-project = Que souhaitez-vous faire dans { $project } ?
conversation-empty-history = Cette conversation ne contient encore aucun message.
conversation-bottom = Aller en bas
conversation-copy = Copier
conversation-compaction = Résumé de la compaction du contexte
conversation-branch-summary = Résumé de la branche
conversation-working = En cours…
conversation-process = Afficher le processus
conversation-details = Afficher ou masquer les détails
tool-detail-offset = Ligne de départ :
tool-detail-line-limit = Nombre de lignes demandé :
tool-detail-timeout = Délai d’expiration (secondes) :
tool-detail-output = Sortie
tool-detail-input = Entrée
tool-detail-additional = Informations supplémentaires
tool-detail-error = Erreur
tool-detail-glob = Motif de fichiers :
tool-detail-ignore-case = Ignorer la casse :
tool-detail-literal = Recherche littérale :
tool-detail-context = Lignes de contexte :
tool-detail-result-limit = Nombre de résultats demandé :
tool-detail-old-text = Texte d’origine demandé
tool-detail-new-text = Texte de remplacement demandé
tool-detail-truncated = Pi a tronqué cette sortie ; le contenu affiché est incomplet.
tool-detail-lines-truncated = Pi a raccourci certaines lignes de résultat.
tool-detail-match-limit = Limite de correspondances atteinte :
tool-detail-results-limited = Limite de résultats atteinte :
tool-detail-entries-limited = Limite d’entrées du répertoire atteinte :
tool-detail-full-output = Fichier de sortie complet :
tool-detail-image-unavailable = Impossible d’afficher cette image.
conversation-role-user = Message de l’utilisateur
conversation-role-assistant = Message de l’assistant
conversation-role-tool = Résultat de l’outil
conversation-event = Événement de session
conversation-close-history = Fermer l’historique des conversations
conversation-source = Afficher le fichier source de la session
conversation-reconnect = Reconnecter
conversation-save-error = Impossible d’enregistrer le brouillon
conversation-interrupted = Exécution arrêtée ; le contenu existant est conservé.
conversation-no = Non
conversation-submit = Envoyer
conversation-input = Écrire un message
conversation-model = Modèle
conversation-load-options = Connectez-vous à Pi pour charger les modèles
conversation-thinking = Niveau de réflexion
conversation-unknown = Indisponible
conversation-context = Contexte de branche de l’exécution actuelle
conversation-auto-compaction = Compaction automatique
conversation-on = Activé
conversation-off = Désactivé
conversation-tokens = Total entrées / sorties
conversation-cache = Total lecture / écriture du cache
conversation-cache-hit = Taux de réussite récent du cache
conversation-cost = Coût indiqué par Pi
conversation-statistics = Statistiques d’utilisation
conversation-send = Envoyer (Entrée) ; Alt+Entrée met une tâche de suivi en file d’attente
conversation-sending = Envoi…
conversation-graph-current = Actuelle
conversation-graph-preview = Aperçu
conversation-graph-left = Afficher les colonnes à gauche (ou faire défiler horizontalement)
conversation-graph-right = Afficher les colonnes à droite (ou faire défiler horizontalement)
conversation-graph-reveal = Afficher la sélection
conversation-graph-column = Graphe
conversation-graph-message = Message
conversation-catalog-empty = Aucune conversation pour le moment

conversation-thinking-content = Réflexion
conversation-thinking-running = Réflexion…
conversation-tool-group = Appels d’outils ({ $count })
conversation-tool-group-read = Fichiers lus ({ $count })
conversation-tool-group-bash = Commandes ({ $count })
conversation-tool-group-search = Recherches ({ $count })
conversation-tool-group-edit = Modifications de fichiers ({ $count })
conversation-tool-line = { $action } { $summary }
conversation-tool-action-read =
    { $state ->
        [running] Lecture en cours
        [complete] Lu
        [failed] Échec de la lecture
       *[unfinished] Lecture inachevée
    }
conversation-tool-action-write =
    { $state ->
        [running] Écriture en cours
        [complete] Écrit
        [failed] Échec de l’écriture
       *[unfinished] Écriture inachevée
    }
conversation-tool-action-edit =
    { $state ->
        [running] Modification en cours
        [complete] Modifié
        [failed] Échec de la modification
       *[unfinished] Modification inachevée
    }
conversation-tool-action-bash =
    { $state ->
        [running] Exécution en cours
        [complete] Exécuté
        [failed] Échec de la commande
       *[unfinished] Exécution inachevée
    }
conversation-tool-action-search =
    { $state ->
        [running] Recherche en cours
        [complete] Recherché
        [failed] Échec de la recherche
       *[unfinished] Recherche inachevée
    }
conversation-tool-action-other =
    { $state ->
        [running] Appel de { $name } en cours
        [complete] { $name } appelé
        [failed] Échec de l’appel de { $name }
       *[unfinished] Appel de { $name } inachevé
    }

conversation-processed = Durée de traitement : { $duration }
conversation-processed-failed = Échec après { $duration }
conversation-processed-stopped = Arrêt après { $duration }
conversation-copied = Copié
conversation-copy-failed = Échec de la copie. Veuillez réessayer.
conversation-usage-title = Utilisation de la requête
conversation-usage-model = Modèle
conversation-usage-provider = Fournisseur
conversation-usage-input = Jetons d’entrée
conversation-usage-output = Jetons de sortie
conversation-usage-cache-read = Jetons lus depuis le cache
conversation-usage-cache-write = Jetons écrits dans le cache
conversation-usage-total = Total des jetons
conversation-usage-cost = Coût

composer-context-used = Jetons utilisés
composer-context-limit = Capacité du contexte
composer-context-percent = Contexte utilisé
composer-token-input = Total des jetons d’entrée
composer-token-output = Total des jetons de sortie
composer-token-cache-read = Total des jetons lus depuis le cache
composer-token-cache-write = Total des jetons écrits dans le cache
composer-token-usage = Utilisation des jetons de la session

conversation-model-search = Rechercher des modèles…
conversation-model-empty = Aucun modèle correspondant
conversation-thinking-off = Désactivé
conversation-thinking-minimal = Minimal
conversation-thinking-low = Faible
conversation-thinking-medium = Moyen
conversation-thinking-high = Élevé
conversation-thinking-xhigh = Très élevé
conversation-thinking-max = Maximum

conversation-model-reasoning = Raisonnement
conversation-model-vision = Vision

conversation-history-reply = Réponse de l’assistant
conversation-history-brief = Bref : messages de l’utilisateur et réponses finales
conversation-history-detailed = Détaillé : messages, outils et résumés
conversation-history-progress = Progression de l’assistant
conversation-history-thinking = Réflexion
conversation-history-calls = Appels d’outils
conversation-history-failed = Échec de l’exécution
conversation-history-stopped = Interrompu
conversation-history-empty = Réponse sans texte
composer-model-thinking = Modèle et niveau de réflexion
composer-model-refresh = Actualiser les modèles et les niveaux de réflexion
composer-thinking-unavailable = Non pris en charge
composer-model-loading = Chargement des modèles…
composer-thinking-loading = Chargement des niveaux de réflexion…
composer-model-confirming = Confirmation des réglages du modèle…
composer-stats-loading = Chargement de l’utilisation…
composer-stats-retry = Réessayer de charger l’utilisation
conversation-checking-file = Vérification du fichier de conversation…
conversation-connecting = Connexion à Pi…
conversation-history-loading = Chargement de l’historique de la conversation…
conversation-history-refreshing = Actualisation de l’historique de la conversation…
conversation-fork-options-loading = Chargement des options de branche…
conversation-fork-options-retry = Réessayer de charger les options de branche
history-canvas-zoom-in = Zoom avant
history-canvas-zoom-out = Zoom arrière
history-canvas-fit = Ajuster l’arbre (0)
history-canvas-current = Localiser le nœud d’exécution
history-canvas-expand = Développer { $count } nœuds
history-canvas-collapse = Réduire { $count } nœuds de processus en conservant les marqueurs de navigation
history-canvas-empty = Aucun nœud d’historique à afficher
history-canvas-help = Arbre de conversation : faites défiler ou glissez pour déplacer, pincez ou utilisez plus/moins pour zoomer, les flèches pour sélectionner, Entrée pour prévisualiser et E pour développer le segment suivant

history-view-tree = Arbre
history-view-list = Liste
history-level-brief = Bref
history-level-detailed = Détaillé
history-level-all = Tout
history-level-all-description = Tout : y compris les réglages, étiquettes et entrées personnalisées
history-scope-all = Toutes les branches
history-scope-branch = Cette branche uniquement
history-model-change = Changement de modèle
history-thinking-change = Changement du niveau de réflexion
history-session-info = Modification des informations de session
history-label-change = Modification de l’étiquette
history-custom-record = Entrée personnalisée


history-content = Contenu
history-range = Portée

command-palette = Palette de commandes
command-focus-input = Placer le focus dans la zone de saisie
command-model = Choisir le modèle et le niveau de réflexion…
command-copy-last-answer = Copier la dernière réponse
command-compact = Compacter le contexte
conversation-compacting = Compaction du contexte…
command-history-description = Afficher l’historique ou choisir un message utilisateur pour créer une branche
command-show-history = Afficher l’historique
command-hide-history = Masquer l’historique
command-current-session = Session actuelle :
command-unavailable = Indisponible dans l’état actuel
command-target-changed = Cette conversation n’est plus disponible. Sélectionnez-en une autre.
command-scanning = Chargement des conversations…
command-loading = Chargement des commandes Pi…
command-no-matches = Aucune commande Pi correspondante
conversation-reconnect-unconfirmed = La fin du processus Pi n’a pas pu être confirmée. Redémarrez Gupi avant de vous reconnecter.
action-retry = Réessayer

command-search-placeholder = Rechercher des actions ou saisir une commande
command-empty = Aucune commande Pi n’est disponible dans cette session

command-group-app = Application
command-group-extensions = Commandes d’extension
command-group-skills = Compétences
command-group-prompts = Modèles d’instructions
command-scope-user = Personnel
command-scope-project = Projet
command-scope-temporary = Temporaire
command-connection-unavailable = Pi n’est pas prêt
command-dismiss = Fermer
command-complete = Terminer
command-send-text = Envoyer
command-execute = Exécuter

command-scope-current = Session actuelle

conversation-working-duration = En cours depuis { $duration }
conversation-tool-group-skill = Compétences lues ({ $count })
conversation-shell-line = { $shell } · { $action } { $summary }
conversation-tool-action-skill =
    { $state ->
        [running] Lecture de la compétence en cours
        [complete] Compétence lue
        [failed] Échec de la lecture de la compétence
       *[unfinished] Lecture de la compétence inachevée
    }

# Unified settings
settings-page-general = Général
settings-page-pi = Pi
settings-page-keys = Raccourcis clavier
settings-page-plugins = Extensions
settings-page-skills = Compétences
settings-page-prompts = Instructions
settings-page-about = À propos
settings-key-reset = Rétablir la valeur par défaut
settings-key-reset-all = Rétablir toutes les valeurs par défaut
settings-key-reset-all-confirm = Rétablir les raccourcis par défaut ? Tous les raccourcis personnalisés seront supprimés.
settings-key-clear = Effacer le raccourci
settings-key-cancel = Annuler les modifications
settings-key-unbound = Aucun raccourci attribué
settings-key-record = Enregistrer
settings-key-recording = Saisissez un raccourci…
settings-key-conflict = Raccourci en conflit ou invalide
settings-key-invalid = Saisissez un raccourci valide ou laissez vide pour supprimer l’attribution.
settings-resource-refresh = Actualiser les ressources
settings-resource-reload-help = Ressources personnelles uniquement. Rechargez manuellement les sessions existantes pour appliquer les modifications.
settings-resource-working = Opération en cours…
settings-resource-pi-required = Enregistrez et vérifiez d’abord un exécutable Pi fonctionnel dans les réglages de Pi.
settings-resource-invalid-name = Les noms peuvent contenir des lettres, des chiffres, des tirets et des traits de soulignement.
settings-resource-register = Ajouter localement
settings-resource-create = Créer
settings-resource-name-help = Nouveau nom de ressource : lettres, chiffres, tirets ou traits de soulignement.
settings-resource-empty = Aucune ressource correspondante.
settings-resource-delete = Déplacer vers la corbeille
settings-resource-save = Enregistrer le texte
shortcut-save-task = Enregistrer
settings-resource-loading = Lecture du texte…
settings-resource-confirm-remove = Supprimer cet élément ? Seul le fichier sélectionné sera déplacé vers la corbeille ; son dossier sera conservé.
settings-package-install = Installer le paquet
settings-package-update = Mettre à jour
settings-package-remove = Supprimer
settings-package-source-help = Source du paquet : npm:name, URL Git ou chemin local absolu.
settings-package-extensions = Extensions { $count }
settings-package-skills = Compétences { $count }
settings-package-prompts = Modèles { $count }
settings-package-themes = Thèmes { $count }
settings-skill-search = Rechercher des compétences par nom, description, chemin ou source
settings-skill-collapse = Réduire
settings-resource-edit = Modifier
settings-about-gupi = Version
settings-about-pi = Version
settings-about-path = Chemin résolu
settings-about-status = État de la vérification
settings-about-unavailable = Pas encore disponible
settings-editor-discard = Abandonner les modifications de texte non enregistrées ?
settings-package-confirm-remove = Supprimer ce paquet des réglages personnels de Pi ? Les dossiers des sources locales seront conservés.
settings-package-heading = Paquets installés
settings-extension-heading = Extensions autonomes
settings-template-heading = Modèles de commandes
settings-system-heading = Instructions système
settings-config-heading = Fichier de configuration
settings-config-open = Ouvrir le fichier de configuration
settings-mode-help = Suivre le système pour basculer automatiquement entre les thèmes clair et sombre.
settings-light-help = Utilisé lorsque le mode clair est actif.
settings-dark-help = Utilisé lorsque le mode sombre est actif.
settings-pi-unsaved = Chemin non enregistré. La vérification valide la saisie actuelle sans l’enregistrer.
settings-pi-saved = Le chemin actuel est enregistré. Vérifiez à nouveau la disponibilité de Pi.
settings-resource-location = Afficher l’emplacement
settings-package-source = Source du paquet
settings-package-empty = Aucun paquet Pi personnel n’est installé.
settings-resource-name = Nom
settings-resource-none = Aucune ressource personnelle de ce type pour le moment.
settings-resource-view = Afficher
settings-resource-readonly-badge = Lecture seule
settings-source-personal = Personnel
settings-source-external = Externe
settings-system-replace = Remplacer l’instruction système par défaut
settings-system-replace-help = Remplace entièrement l’instruction système par défaut de Pi.
settings-system-append = Ajouter des instructions personnelles
settings-system-append-help = Conserve l’instruction système par défaut et ajoute ces instructions.
settings-resource-success = Opération terminée pour « { $target } ». Rechargez manuellement les sessions existantes.
settings-resource-failed = Échec de l’opération pour « { $target } ».
settings-about-help = Affiche le dernier contrôle du chemin enregistré. Ouvrez les réglages de Pi pour le vérifier à nouveau.
settings-key-reveal-session = Afficher le fichier de session

settings-key-group-app = Application
settings-key-group-conversation = Conversation
settings-key-group-files = Fichiers de session

settings-package-kind-extensions = Extensions
settings-package-kind-skills = Compétences
settings-package-kind-themes = Thèmes
settings-package-kind-prompts = Modèles
settings-package-expand = Développer le contenu du paquet
settings-package-collapse = Réduire le contenu du paquet

settings-template-search = Rechercher des noms, descriptions ou sources de modèles
settings-template-collapse = Réduire l’aperçu

temporary-title = Conversations temporaires
attachment-add = Joindre des fichiers
attachment-remove = Supprimer la pièce jointe
attachment-file = Fichier
attachment-clipboard = Image du presse-papiers
image-preview-close = Fermer l’aperçu
image-preview-open = Ouvrir l’aperçu de l’image
image-preview-zoom-in = Agrandir l’image
image-preview-zoom-out = Réduire l’image
shortcut-global = Raccourcis globaux
shortcut-launcher = Fenêtre temporaire
shortcut-add = Ajouter une tâche modèle
shortcut-edit = Modifier la tâche modèle
shortcut-delete = Supprimer la tâche modèle
shortcut-name = Nom
shortcut-template = Modèle de commande
shortcut-source = Source de saisie
shortcut-selection = Texte sélectionné
shortcut-clipboard = Presse-papiers
shortcut-fallback = Texte sélectionné, sinon presse-papiers
shortcut-model = Modèle
shortcut-thinking = Niveau de réflexion
shortcut-default-model = Modèle par défaut de Pi
shortcut-default-thinking = Niveau de réflexion par défaut de Pi
shortcut-personal = Personnel
shortcut-template-required = Sélectionnez un modèle de commande.
shortcut-reload-options = Recharger les options
temporary-clean-released = Espaces de travail libérés
temporary-clean = Nettoyer
temporary-clean-help = Déplacer les espaces de travail temporaires libérés vers la corbeille. Les conversations actives sont conservées.
temporary-cleaned = Espaces de travail déplacés vers la corbeille
shortcut-tasks = Tâches modèles

shortcut-select-template = Choisir un modèle d’instructions

temporary-search-placeholder = Rechercher des conversations temporaires
temporary-search-empty = Aucune conversation temporaire correspondante
temporary-toggle-input = Changer de mode de saisie
temporary-hide = Masquer
session-search-open = Ouvrir la conversation

temporary-actions = Actions
temporary-paste-answer = Coller la dernière réponse
temporary-reveal-workspace = Afficher le dossier de travail
temporary-search-actions = Rechercher des actions…
temporary-switch-session = Changer de conversation temporaire
temporary-paste-failed = Réponse copiée. Le collage automatique a échoué. Vérifiez l’autorisation d’accessibilité et l’application cible, ou collez le texte manuellement.
temporary-session-1 = Passer à la conversation temporaire 1
temporary-session-2 = Passer à la conversation temporaire 2
temporary-session-3 = Passer à la conversation temporaire 3
temporary-session-4 = Passer à la conversation temporaire 4
temporary-session-5 = Passer à la conversation temporaire 5
temporary-session-6 = Passer à la conversation temporaire 6
temporary-session-7 = Passer à la conversation temporaire 7
temporary-session-8 = Passer à la conversation temporaire 8
temporary-session-9 = Passer à la dernière conversation temporaire
temporary-send = Envoyer
temporary-trash = Déplacer la conversation temporaire vers la corbeille
settings-key-stop-or-hide = Arrêter la génération / masquer la fenêtre temporaire

conversation-queue-title = En attente ({ $count })
conversation-queue-steer = Orienter le tour en cours
conversation-queue-follow-up = Tâche de suivi
conversation-queue-restore = Récupérer tout le texte en attente dans le brouillon
conversation-queue-clear = Effacer tous les messages en attente
conversation-queue-no-text = Message sans texte
conversation-queue-unavailable = Le contenu de la file d’attente n’est pas encore disponible
conversation-queue-text-only = La récupération de la file restaure uniquement le texte ; les images en attente ne peuvent pas être restaurées.

conversation-retry-countdown = Nouvel essai { $attempt }/{ $total } dans environ { $seconds } s
conversation-retry-waiting = Nouvel essai { $attempt }/{ $total } : en attente de Pi
conversation-summary-retry-countdown = Nouvel essai du résumé { $attempt }/{ $total } dans environ { $seconds } s
conversation-summary-retry-waiting = Nouvel essai du résumé { $attempt }/{ $total } : en attente de Pi

message-details-open = Afficher les détails…
message-details-copy-all = Tout copier
tool-detail-command = Commande
tool-detail-content = Contenu
tool-detail-changes = Modifications

settings-notifications = Notifications
settings-notification-help = Les notifications système utilisent une description générique. Ouvrez une notification pour revenir à sa conversation. Les compteurs de messages non lus sont effacés lorsque la conversation est ouverte au premier plan.
settings-notification-waiting = Notifier lorsqu’une saisie est nécessaire en arrière-plan
settings-notification-failures = Notifier lorsqu’une tâche échoue en arrière-plan
settings-notification-plugins = Envoyer les rappels des extensions comme notifications système en arrière-plan
settings-notification-attention = Demander votre attention lorsqu’une saisie est requise en arrière-plan
settings-notification-completion = Notifications de réponse terminée
notification-off = Désactivé
notification-background = Arrière-plan uniquement
notification-always = Toujours
notification-waiting = En attente de votre saisie
notification-completed = Votre réponse est prête
notification-failed = Une tâche nécessite votre attention après un échec
notification-plugin = Rappels des extensions
notification-unread = Conversations non lues
notification-running = En cours
notification-clear = Effacer les rappels
notification-session-notices = Rappels de conversation

settings-icon-theme = Icône de l’application
settings-icon-theme-help = Modifie le logo dans l’application et l’icône du Dock de l’application macOS en cours d’exécution. Finder et la barre des tâches Windows conservent l’icône empaquetée.
icon-theme-classic = Classique
icon-theme-classic-gradient = Classique dégradé
icon-theme-color = Couleurs Pi
icon-theme-color-gradient = Dégradé Pi
icon-theme-pride = Arc-en-ciel
icon-theme-ukraine = Ukraine
icon-theme-ukraine-gradient = Ukraine dégradé

language-traditional-chinese = 繁體中文
language-japanese = 日本語
language-korean = 한국어
language-german = Deutsch
language-french = Français
language-spanish = Español
language-portuguese-brazil = Português (Brasil)
menu-about = À propos de Gupi
menu-conversation = Conversation
menu-edit = Édition
menu-view = Présentation
menu-window = Fenêtre
menu-help = Aide
menu-services = Services
menu-hide = Masquer Gupi
menu-hide-others = Masquer les autres
menu-show-all = Tout afficher
menu-undo = Annuler
menu-redo = Rétablir
menu-cut = Couper
menu-copy = Copier
menu-paste = Coller
menu-select-all = Tout sélectionner
menu-minimize = Réduire
menu-zoom = Zoom
menu-fullscreen = Activer ou désactiver le plein écran
menu-docs = Guide de l’utilisateur de Gupi
menu-pi-docs = Documentation de Pi
menu-report-issue = Signaler un problème
menu-logs = Afficher les journaux
menu-copy-diagnostics = Copier les diagnostics
settings-permissions = Autorisations système
settings-accessibility-help = La sélection de texte et le collage dans une autre application peuvent nécessiter l’accès à l’accessibilité. Activez Gupi dans Réglages Système, puis réessayez. La réponse reste disponible à copier si le collage échoue.
settings-accessibility-open = Réglages d’accessibilité
settings-notification-permission-help = Si les alertes ou le badge du Dock n’apparaissent pas, autorisez les notifications et les badges de Gupi dans Réglages Système. L’autorisation est demandée lors de la première utilisation de la fonctionnalité.
settings-notification-permission-open = Réglages des notifications
settings-native-language-help = La langue de l’application change immédiatement. Redémarrez Gupi pour l’appliquer aux dialogues système de macOS. Les dialogues système de Windows suivent la langue d’affichage de Windows.
settings-diagnostics-help = Inclut la version de l’application, la plateforme, la commande Pi et les emplacements des fichiers de configuration et de journaux. N’inclut pas les conversations, les identifiants ni les variables d’environnement.

setup-preferences-title = Langue et apparence
setup-preferences-step = Langue et apparence
setup-desktop-title = Raccourcis et notifications
setup-desktop-step = Raccourcis et notifications
setup-desktop-help = Vous pourrez modifier ces choix plus tard dans les réglages.
setup-page-skip = Ignorer cette page
setup-skip-all = Utiliser les valeurs par défaut
setup-pi-later = Configurer Pi plus tard
action-open = Ouvrir

setup-appearance-options = Choisir un thème et une icône
setup-pi-install-guide = Installer Pi
setup-pi-model-guide = Connecter un modèle

conversation-plugin-message = Message du plug-in
conversation-session-info-command = Informations de la conversation…
conversation-session-info-title = Informations de la conversation
conversation-session-info-identity = Identité et emplacement
conversation-session-info-name = Conversation
conversation-session-info-id = ID de conversation Pi
conversation-session-info-directory = Répertoire de travail
conversation-session-info-file = Fichier de conversation
conversation-session-info-not-saved = Pas encore enregistrée
conversation-session-info-runtime = Configuration d’exécution
conversation-session-info-model = Modèle
conversation-session-info-thinking = Niveau de réflexion
conversation-session-info-usage = Utilisation
conversation-session-info-input = Jetons d’entrée
conversation-session-info-output = Jetons de sortie
conversation-session-info-cache-read = Lectures du cache
conversation-session-info-cache-write = Écritures du cache
conversation-session-info-cost = Coût indiqué par Pi
conversation-session-info-context = Contexte actuel (jetons / fenêtre / utilisation)
conversation-session-info-context-value = { $tokens } sur { $window } jetons ({ $percent })
conversation-session-info-counts = Statistiques de la conversation
conversation-session-info-user-messages = Messages utilisateur
conversation-session-info-assistant-messages = Messages de l’assistant
conversation-session-info-tool-calls = Appels d’outils
conversation-session-info-tool-results = Résultats d’outils
conversation-session-info-total-messages = Nombre total de messages
