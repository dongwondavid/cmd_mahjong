use serde::{Deserialize, Serialize};

use super::parse::*;
use super::point::*;
use super::yaku::*;
use crate::model::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct WinContext {
    pub yakus: Vec<(String, usize)>, // 役一覧(ドラは含まない), Vec<(name, fan)>
    pub is_tsumo: bool,              // true: ツモ, false: ロン
    pub score_title: String,         // 倍満, 跳満, ...
    pub n_dora: usize,               // 通常のドラの数
    pub n_red_dora: usize,           // 赤ドラの数
    pub n_ura_dora: usize,           // 裏ドラの数
    pub fu: usize,                   // 符数
    pub fan: usize,                  // 翻数(ドラを含む), 役満倍率(is_yakuman=trueの時)
    pub yakuman_times: usize,        // 役満倍率 (0: 通常役, 1: 役満, 2: 二倍役満, ...)
    pub points: Points,              // 支払い得点
}

pub fn evaluate_hand_tsumo(stage: &Stage, ura_dora_wall: &Vec<Tile>) -> Option<WinContext> {
    let pl = &stage.players[stage.turn];
    if !pl.is_shown {
        return None;
    }

    if !pl.win_tiles.contains(&pl.drawn.unwrap().to_normal()) {
        return None;
    }

    let mut yf = YakuFlags::default();
    yf.menzentsumo = pl.melds.is_empty();
    yf.riichi = pl.is_riichi && !pl.is_daburii;
    yf.dabururiichi = pl.is_daburii;
    yf.ippatsu = pl.is_ippatsu;
    yf.haiteiraoyue = stage.left_tile_count == 0;
    yf.rinshankaihou = pl.is_rinshan;
    yf.tenhou = false;
    yf.tiihou = false;

    let ura_doras = if !ura_dora_wall.is_empty() && pl.is_riichi {
        ura_dora_wall[0..stage.doras.len()].to_vec()
    } else {
        vec![]
    };

    if let Some(res) = evaluate_hand(
        &pl.hand,
        &pl.melds,
        &stage.doras,
        &ura_doras,
        pl.drawn.unwrap(),
        true,
        stage.is_leader(pl.seat),
        stage.get_prevalent_wind(),
        stage.get_seat_wind(pl.seat),
        yf,
    ) {
        if !res.yakus.is_empty() {
            return Some(res);
        }
    }

    None
}

pub fn evaluate_hand_ron(
    stage: &Stage,
    ura_dora_wall: &Vec<Tile>,
    seat: Seat,
) -> Option<WinContext> {
    if seat == stage.turn {
        return None;
    }

    let pl = &stage.players[seat];
    if let Some((_, _, t)) = stage.last_tile {
        if !pl.win_tiles.contains(&t.to_normal()) {
            return None;
        }
    }
    if !pl.is_shown || pl.is_furiten || pl.is_furiten_other {
        return None;
    }

    let mut yf = YakuFlags::default();
    yf.riichi = pl.is_riichi && !pl.is_daburii;
    yf.dabururiichi = pl.is_daburii;
    yf.ippatsu = pl.is_ippatsu;
    let (tp, t) = if let Some((_, tp, t)) = stage.last_tile {
        match tp {
            ActionType::Discard => yf.houteiraoyui = stage.left_tile_count == 0,
            ActionType::Kakan => yf.chankan = true,
            ActionType::Ankan => {}
            _ => panic!(),
        }
        (tp, t)
    } else {
        return None;
    };

    let mut hand = pl.hand.clone();
    if t.1 == 0 {
        // 赤5
        hand[t.0][0] += 1;
        hand[t.0][5] += 1;
    } else {
        hand[t.0][t.1] += 1;
    }

    let ura_doras = if !ura_dora_wall.is_empty() && pl.is_riichi {
        ura_dora_wall[0..stage.doras.len()].to_vec()
    } else {
        vec![]
    };

    if let Some(res) = evaluate_hand(
        &hand,
        &pl.melds,
        &stage.doras,
        &ura_doras,
        t,
        false,
        stage.is_leader(pl.seat),
        stage.get_prevalent_wind(),
        stage.get_seat_wind(pl.seat),
        yf,
    ) {
        if tp == ActionType::Ankan {
            for y in &res.yakus {
                if y.0 == "国士無双" || y.0 == "国士無双十三面待ち" {
                    return Some(res);
                }
            }
        } else if !res.yakus.is_empty() {
            return Some(res);
        }
    }

    None
}

// 和了形である場合,最も高得点となるような役の組み合わせのSome(Result)を返却
// 和了形でない場合,Noneを返却
// 和了形でも無役の場合はResultの中身がyaku: [], points(0, 0, 0)となる.
pub fn evaluate_hand(
    hand: &TileTable,      // 手牌(鳴き以外)
    melds: &Vec<Meld>,     // 鳴き
    doras: &Vec<Tile>,     // ドラ表示牌 (注:ドラそのものではない)
    ura_doras: &Vec<Tile>, // 裏ドラ表示牌 リーチしていない場合は空
    win_tile: Tile,        // 上がり牌
    is_tsumo: bool,        // ツモ和了
    is_leader: bool,       // 親番
    prevalent_wind: Tnum,  // 場風 (東: 1, 南: 2, 西: 3, 北: 4)
    seat_wind: Tnum,       // 自風 (同上)
    yaku_flags: YakuFlags, // 和了形だった場合に自動的に付与される役(特殊条件役)のフラグ
) -> Option<WinContext> {
    let mut wins = vec![];

    // 和了(通常)
    let pm = parse_melds(melds);
    for mut ph in parse_into_normal_win(hand).into_iter() {
        ph.append(&mut pm.clone());
        let ctx = YakuContext::new(
            hand.clone(),
            ph,
            win_tile,
            prevalent_wind,
            seat_wind,
            is_tsumo,
            yaku_flags.clone(),
        );
        wins.push(ctx);
    }

    // 和了(七対子)
    for ph in parse_into_chiitoitsu_win(hand).into_iter() {
        let ctx = YakuContext::new(
            hand.clone(),
            ph,
            win_tile,
            prevalent_wind,
            seat_wind,
            is_tsumo,
            yaku_flags.clone(),
        );
        wins.push(ctx);
    }

    // 和了(国士無双)
    for ph in parse_into_kokusimusou_win(hand).into_iter() {
        let ctx = YakuContext::new(
            hand.clone(),
            ph,
            win_tile,
            prevalent_wind,
            seat_wind,
            is_tsumo,
            yaku_flags.clone(),
        );
        wins.push(ctx);
    }

    if wins.is_empty() {
        return None; // 和了形以外
    }

    let n_dora = count_dora(hand, melds, doras);
    let mut n_red_dora = hand[0][0] + hand[1][0] + hand[2][0];
    for m in melds {
        for t in &m.tiles {
            if t.1 == 0 {
                n_red_dora += 1;
            }
        }
    }
    let n_ura_dora = if yaku_flags.riichi || yaku_flags.dabururiichi {
        count_dora(hand, melds, ura_doras)
    } else {
        0
    };

    let mut results = vec![];
    for ctx in wins {
        let fu = ctx.calc_fu();
        let (yakus, mut fan, yakuman_times) = ctx.calc_yaku();
        if yakuman_times == 0 {
            fan += n_dora + n_red_dora + n_ura_dora;
        }
        let points = if yakus.is_empty() {
            (0, 0, 0) // 役無し
        } else {
            get_points(is_leader, fu, fan, yakuman_times)
        };
        let yakus: Vec<(String, usize)> = yakus
            .iter()
            .map(|y| {
                let fan = if ctx.is_open() {
                    y.fan_open
                } else {
                    y.fan_close
                };
                (y.name.to_string(), fan)
            })
            .collect();
        let score_title = get_score_title(fu, fan, yakuman_times);
        results.push(WinContext {
            yakus,
            is_tsumo,
            score_title,
            n_dora,
            n_red_dora,
            n_ura_dora,
            fu,
            fan,
            yakuman_times,
            points,
        });
    }

    results.sort_by_key(|r| r.points.0);
    results.pop()
}

// ドラ表示牌のリストを受け取ってドラ評価値のテーブルを返却
fn create_dora_table(doras: &Vec<Tile>) -> TileTable {
    let mut dt = TileTable::default();
    for d in doras {
        let ni = if d.is_hornor() {
            match d.1 {
                WN => WE,
                DR => DW,
                i => i + 1,
            }
        } else {
            match d.1 {
                9 => 1,
                0 => 6,
                _ => d.1 + 1,
            }
        };
        dt[d.0][ni] += 1;
    }

    dt
}

// ドラ(赤5を含む)の数を勘定
fn count_dora(hand: &TileTable, melds: &Vec<Meld>, doras: &Vec<Tile>) -> usize {
    let dt = create_dora_table(doras);
    let mut n_dora = 0;

    for ti in 0..TYPE {
        for ni in 1..TNUM {
            n_dora += dt[ti][ni] * hand[ti][ni];
        }
    }

    for m in melds {
        for t in &m.tiles {
            let t = t.to_normal();
            n_dora += dt[t.0][t.1];
        }
    }

    n_dora
}
