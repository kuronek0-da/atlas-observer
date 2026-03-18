/*
From CharacterSelect.cpp in CCCaster
uint8_t charaToSelector ( uint8_t chara )
{
    switch ( chara )
    {
        // First row
        case 22: return  2; // Aoko
        case  7: return  3; // Tohno
        case 51: return  4; // Hime
        case 15: return  5; // Nanaya
        case 28: return  6; // Kouma
        // Second row
        case  8: return 10; // Miyako
        case  2: return 11; // Ciel
        case  0: return 12; // Sion
        case 30: return 13; // Ries
        case 11: return 14; // V.Sion
        case  9: return 15; // Wara
        case 31: return 16; // Roa
        // Third row
        case  4: return 19; // Maids
        case  3: return 20; // Akiha
        case  1: return 21; // Arc
        case 19: return 22; // P.Ciel
        case 12: return 23; // Warc
        case 13: return 24; // V.Akiha
        case 14: return 25; // M.Hisui
        // Fourth row
        case 29: return 28; // S.Akiha
        case 17: return 29; // Satsuki
        case 18: return 30; // Len
        case 33: return 31; // Ryougi
        case 23: return 32; // W.Len
        case 10: return 33; // Nero
        case 25: return 34; // NAC
        // Firth row
        case 35: return 38; // KohaMech
        case  5: return 39; // Hisui
        case 20: return 40; // Neko
        case  6: return 41; // Kohaku
        case 34: return 42; // NekoMech
        // Last row
        case RANDOM_CHARACTER: return RANDOM_CHARA_SELECTOR;
    }

    return UNKNOWN_POSITION;
}
*/

use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, TryFromPrimitive, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[repr(u32)]
pub enum GameChar {
    Aoko = 22,
    Tohno = 7,
    Hime = 51,
    Nanaya = 15,
    Kouma = 28,
    Miyako = 8,
    Ciel = 2,
    Sion = 0,
    Ries = 30,
    VSion = 11,
    Wara = 9,
    Roa = 31,
    Maids = 4,
    Akiha = 3,
    Arc = 1,
    Pciel = 19,
    Warc = 12,
    Vakiha = 13,
    Mech = 14,
    Seifuku = 29,
    Satsuki = 17,
    Len = 18,
    Ryougi = 33,
    Wlen = 23,
    Nero = 10,
    Nac = 25,
    Kohamech = 35,
    Hisui = 5,
    Neko = 20,
    Kohaku = 6,
    Nekomech = 34,
}

#[derive(Debug, PartialEq, Eq, Clone, TryFromPrimitive, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[repr(u32)]
pub enum Moon {
    Crescent,
    Full,
    Half
}
