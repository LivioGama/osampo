#pragma once
#include "data.h"
#include "json_line.h"

// Reporte les cles presentes dans la ligne sur la trame courante.
// Une cle absente laisse la valeur precedente : c est la regle du contrat,
// d ou le passage de la valeur courante en repli a chaque accesseur.
void decode(const JsonLine &line, DataFrame &d);
