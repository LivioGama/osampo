# strawheart, « Animals and Tea » : utilisé pour les poses de face du chiot

- **URL :** https://opengameart.org/content/animals-and-tea-by-strawheart
  (https://opengameart.org/node/6309).
- **Auteur :** strawheart, page publiée sur OpenGameArt le 31 décembre 2011.
- **Licence lue sur la page le 2026-11-20 :** CC-BY-SA 3.0
  (https://creativecommons.org/licenses/by-sa/3.0/).
- **Attribution demandée sur la page :** « If you use, just provide a link to my
  website: http://mtsids.com/ ».
- **Fichier :** `pixel-ani-dog.gif`, téléchargé le 2026-11-20 depuis
  https://opengameart.org/sites/default/files/pixel-ani-dog.gif, 23 x 25, 444 octets,
  sha256 `e5ad6345d0f5e15331b39dd1cec26ba2c80eef08ec43e97069a64c3a2c129741`.
  GIF animé de 2 images à 200 ms : chiot assis de face, queue en bas puis levée,
  8 couleurs. Commité tel quel, non modifié.

Les autres fichiers du pack (chat, perruches, serpent, tasses) n'ont pas été importés.

## Ce qui est extrait et modifié

`tools/gen_assets.py` (`anim_repos`) lit les deux images du GIF et produit
`ui/sprites/repos.png` :

- agrandissement x2 au plus proche voisin (23 x 25 vers 46 x 50), couleurs
  d'origine gardées ;
- yeux redessinés au pixel de la case (2 x 4, reflet blanc en haut à gauche,
  comme les autres poses du widget) au lieu du pixel noir agrandi ;
- pose dans la case 64 x 64, corps centré, pattes sur la ligne de sol du widget ;
- ajout d'une ombre au sol semi-transparente, comme les autres poses ;
- séquence de 8 cases à 150 ms : deux battements de queue, une pause et un
  clignement (case 6, yeux fermés en arc).

Autres poses de face dérivées de la même source, par retouches au pixel d'origine
avant agrandissement puis au pixel de la case :

| Planche | Fonction | Retouches |
|---|---|---|
| `patte.png` | `anim_patte` | patte posée contre l'écran, coussinets (couleur ajoutée) |
| `content.png` | `anim_content` | yeux rieurs, langue, petits sauts, cœurs |
| `aboie.png` | `anim_aboie` | gueule ouverte, traits d'aboiement |
| `boude.png` | `anim_boude` | yeux tristes, nuage de pluie |
| `tournis.png` | `anim_tournis` | yeux en X, étoiles, balancement |
| `laisse.png` | `anim_laisse` | collier rouge, laisse |
| `lache.png` | `anim_lache` | gueule ouverte, langue, balle qui tombe, cœurs |
| `bain.png`, `bain_mousse.png`, `bain_fin.png` | `anim_bain*` | bain, mousse, ébrouement |
| `os.png`, `os_fin.png` | `anim_os*` | os tenu, rongé |

## Licence des dérivés

Sont des œuvres dérivées, distribuées sous **CC-BY-SA 3.0** avec attribution à
strawheart (http://mtsids.com/) :

- `ui/sprites/repos.png`, `patte.png`, `content.png`, `aboie.png`, `boude.png`,
  `tournis.png`, `laisse.png`, `lache.png`, `bain.png`, `bain_mousse.png`,
  `bain_fin.png`, `os.png`, `os_fin.png` ;
- `tools/planche_contact.png`, qui les reproduit.

## Poses de profil : pas de pixels de cette source

`dort.png`, `court.png`, `court_droite.png`, `balle.png` et `roulade.png` montrent un
chiot de profil dessiné à neuf dans `tools/gen_assets.py` (tête de profil, corps et
pattes originaux). Seule la palette de couleurs de strawheart est reprise, pour que
les deux vues aillent ensemble. Des images de référence (photos, illustrations et
croquis de chiens) ont servi de modèles de poses, sans copie de pixels ; aucune
n'est commitée. La niche, les fonds et les icônes sont aussi des dessins originaux.
