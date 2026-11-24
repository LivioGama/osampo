#pragma once
// Migration de l'espace NVS du nom de travail du projet vers "osampo" : le
// chiot d'une board flashee avant le renommage garde son etat.
#include <string>
#include <vector>

namespace board_183_sq {

constexpr const char *kEspaceAncien = "tagamoshi";
constexpr const char *kEspace = "osampo";

// Cles de l'ancien espace absentes du nouveau (dans l'ordre de `anciennes`).
std::vector<std::string> cles_a_migrer(const std::vector<std::string> &anciennes,
                                       const std::vector<std::string> &nouvelles);

// Cles de l'ancien espace a effacer une fois le nouveau commite : toutes sauf celles dont la
// copie a echoue. Les cles deja presentes dans le nouvel espace partent aussi (la valeur du
// nouvel espace fait foi). Effacer au fur et a mesure libere la NVS : une migration
// interrompue par une NVS pleine avance au demarrage suivant au lieu de rester bloquee.
std::vector<std::string> cles_a_effacer(const std::vector<std::string> &anciennes,
                                        const std::vector<std::string> &echouees);

} // namespace board_183_sq
