idf_component_register(
    SRCS "main.cpp"
         "../../src/decode.cpp"
         "../../src/logic/controller.cpp"
         "../../src/logic/pup.cpp"
         "../../src/logic/reminders.cpp"
         "../../src/logic/clock.cpp"
         "../../src/logic/needs.cpp"
         "../../src/logic/settings.cpp"
         "../../src/logic/rappel.cpp"
         "../../src/logic/lien_pc.cpp"
         "../../src/logic/gestures.cpp"
         "../../src/logic/allure.cpp"
         "../../src/logic/volume.cpp"
         "../../../../platform/link_common/json_line.cpp"
    INCLUDE_DIRS "." "../../src" "../../../../platform/link_common"
    REQUIRES slint json board_183_sq
    EMBED_FILES "../../sounds/wouf.wav" "../../sounds/aboie.wav"
                "../../sounds/gemit.wav" "../../sounds/jappe.wav"
)
slint_target_sources(${COMPONENT_LIB} ../../ui/view_square.slint)
