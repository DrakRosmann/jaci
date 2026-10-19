// Jaci: ponte entre o kOS e o centro de controle Jaci.
//
// Grava a telemetria em 0:/jaci/tm-<id>.txt e lê os comandos de
// 0:/jaci/cmd-<id>.txt. O volume 0 é a pasta Ships/Script do KSP, que o Jaci
// lê e escreve no mesmo computador. Sem conexão com o KSC (CommNet), não há
// telemetria nem comandos, como numa perda de sinal.
//
// Rode com RUNPATH("0:/jaci/jaci.ks"). ou escolha boot/jaci.ks como arquivo
// de boot do computador kOS no VAB.

@LAZYGLOBAL OFF.
SET CONFIG:IPU TO 2000.
CLEARSCREEN.
PRINT "Jaci: ponte de telemetria e comandos".

LOCAL uid IS CORE:PART:UID.
LOCAL tmf IS "0:/jaci/tm-" + uid + ".txt".
LOCAL cmdf IS "0:/jaci/cmd-" + uid + ".txt".
LOCAL lastSeq IS -1.
LOCAL session IS "".
LOCAL seq IS 0.
LOCAL nlines IS 0.
LOCAL tick IS 0.
LOCAL lastBody IS "".
LOCAL lastStage IS STAGE:NUMBER.

// Piloto automático: manual, ascent (subida), coast (até sair da
// atmosfera), node (executa a próxima manobra).
LOCAL mode IS "manual".
LOCAL steerMode IS "free".
LOCAL steerLocked IS FALSE.
LOCAL steerDir IS SHIP:FACING.
LOCAL holdDir IS SHIP:FACING.
LOCAL steerHdg IS 90.
LOCAL steerPitch IS 90.
LOCAL thr IS 0.
LOCAL thrLocked IS FALSE.
LOCAL ascentApo IS 80000.
LOCAL ascentHdg IS 90.
LOCAL burning IS FALSE.
LOCAL burnVec0 IS V(0, 0, 0).

FUNCTION linked {
  RETURN HOMECONNECTION:ISCONNECTED.
}

FUNCTION out {
  PARAMETER s.
  IF linked() {
    LOG s TO tmf.
    SET nlines TO nlines + 1.
  }
}

FUNCTION header {
  out("H " + uid + " " + SHIP:NAME).
  bodyLine().
}

FUNCTION bodyLine {
  LOCAL atmh IS 0.
  IF BODY:ATM:EXISTS { SET atmh TO BODY:ATM:HEIGHT. }
  out("B " + BODY:NAME:REPLACE(" ", "_") + " " + BODY:RADIUS + " " + BODY:MU + " " + atmh + " " + BODY:ROTATIONPERIOD).
  SET lastBody TO BODY:NAME.
}

// Arquivo novo de tempos em tempos para não crescer sem fim.
FUNCTION rotate {
  IF nlines > 3000 AND linked() {
    DELETEPATH(tmf).
    SET nlines TO 0.
    header().
  }
}

FUNCTION telemetry {
  LOCAL e IS LIST().
  LIST ENGINES IN e.
  LOCAL thrustNow IS 0.
  FOR x IN e {
    IF x:IGNITION { SET thrustNow TO thrustNow + x:THRUST. }
  }
  LOCAL lf IS 0. LOCAL lfc IS 0.
  LOCAL ox IS 0. LOCAL oxc IS 0.
  LOCAL mp IS 0. LOCAL mpc IS 0.
  LOCAL ec IS 0. LOCAL ecc IS 0.
  LOCAL sf IS 0. LOCAL sfc IS 0.
  FOR r IN SHIP:RESOURCES {
    IF r:NAME = "LiquidFuel" { SET lf TO r:AMOUNT. SET lfc TO r:CAPACITY. }
    ELSE IF r:NAME = "Oxidizer" { SET ox TO r:AMOUNT. SET oxc TO r:CAPACITY. }
    ELSE IF r:NAME = "MonoPropellant" { SET mp TO r:AMOUNT. SET mpc TO r:CAPACITY. }
    ELSE IF r:NAME = "ElectricCharge" { SET ec TO r:AMOUNT. SET ecc TO r:CAPACITY. }
    ELSE IF r:NAME = "SolidFuel" { SET sf TO r:AMOUNT. SET sfc TO r:CAPACITY. }
  }
  LOCAL fwd IS SHIP:FACING:FOREVECTOR.
  LOCAL upv IS SHIP:UP:VECTOR.
  LOCAL northv IS SHIP:NORTH:VECTOR.
  LOCAL eastv IS VCRS(upv, northv).
  LOCAL pitch IS 90 - VANG(fwd, upv).
  LOCAL hdg IS ARCTAN2(VDOT(fwd, eastv), VDOT(fwd, northv)).
  IF hdg < 0 { SET hdg TO hdg + 360. }
  LOCAL ndt IS -1.
  LOCAL ndv IS 0.
  IF HASNODE { SET ndt TO NEXTNODE:ETA. SET ndv TO NEXTNODE:DELTAV:MAG. }
  LOCAL tgt IS "-".
  LOCAL tdist IS -1.
  IF HASTARGET { SET tgt TO TARGET:NAME:REPLACE(" ", "_"). SET tdist TO TARGET:DISTANCE. }
  LOCAL o IS SHIP:ORBIT.
  // Em hipérbole o período e o apoastro não existem (o kOS pararia com NaN).
  LOCAL closed IS o:ECCENTRICITY < 1.
  LOCAL per IS -1.
  LOCAL etaAp IS -1.
  LOCAL etaPe IS -1.
  IF closed {
    SET per TO o:PERIOD.
    SET etaAp TO ETA:APOAPSIS.
    SET etaPe TO ETA:PERIAPSIS.
  } ELSE IF VDOT(SHIP:VELOCITY:ORBIT, -BODY:POSITION) < 0 {
    SET etaPe TO ETA:PERIAPSIS.
  }
  // Ordem fixa: o Jaci lê pela posição (docs/protocolo.md).
  out("T " + seq
    + " " + ROUND(TIME:SECONDS, 2) + " " + ROUND(MISSIONTIME, 2) + " " + SHIP:STATUS
    + " " + ROUND(ALTITUDE, 1) + " " + ROUND(SHIP:LATITUDE, 5) + " " + ROUND(SHIP:LONGITUDE, 5)
    + " " + ROUND(SHIP:VELOCITY:ORBIT:MAG, 2) + " " + ROUND(SHIP:VELOCITY:SURFACE:MAG, 2) + " " + ROUND(SHIP:VERTICALSPEED, 2)
    + " " + ROUND(APOAPSIS, 1) + " " + ROUND(PERIAPSIS, 1) + " " + ROUND(o:INCLINATION, 4) + " " + ROUND(o:ECCENTRICITY, 6)
    + " " + ROUND(per, 1) + " " + ROUND(o:LAN, 4) + " " + ROUND(o:ARGUMENTOFPERIAPSIS, 4) + " " + ROUND(o:TRUEANOMALY, 4)
    + " " + ROUND(o:SEMIMAJORAXIS, 1)
    + " " + ROUND(SHIP:MASS, 4) + " " + ROUND(thrustNow, 2) + " " + ROUND(SHIP:MAXTHRUST, 2) + " " + ROUND(SHIP:CONTROL:PILOTMAINTHROTTLE, 3)
    + " " + ROUND(SHIP:Q * 101.325, 3) + " " + STAGE:NUMBER
    + " " + ROUND(lf, 2) + " " + ROUND(lfc, 2) + " " + ROUND(ox, 2) + " " + ROUND(oxc, 2)
    + " " + ROUND(mp, 2) + " " + ROUND(mpc, 2) + " " + ROUND(ec, 2) + " " + ROUND(ecc, 2)
    + " " + ROUND(sf, 2) + " " + ROUND(sfc, 2)
    + " " + ROUND(pitch, 2) + " " + ROUND(hdg, 2)
    + " " + SAS + " " + RCS + " " + GEAR + " " + LIGHTS + " " + BRAKES
    + " " + ROUND(etaAp, 1) + " " + ROUND(etaPe, 1)
    + " " + ROUND(ndt, 1) + " " + ROUND(ndv, 2) + " " + tgt + " " + ROUND(tdist, 1)
    + " " + WARP + " " + SHIP:CREW():LENGTH + " " + mode + " " + steerMode).
  SET seq TO seq + 1.
}

// ── Controle ──

FUNCTION steerOn {
  IF NOT steerLocked {
    SAS OFF.
    LOCK STEERING TO steerDir.
    SET steerLocked TO TRUE.
  }
}

FUNCTION throttleOn {
  IF NOT thrLocked {
    LOCK THROTTLE TO thr.
    SET thrLocked TO TRUE.
  }
}

FUNCTION release {
  UNLOCK STEERING.
  UNLOCK THROTTLE.
  SET steerLocked TO FALSE.
  SET thrLocked TO FALSE.
  SET burning TO FALSE.
  SET steerMode TO "free".
}

FUNCTION normalVec {
  RETURN VCRS(SHIP:VELOCITY:ORBIT, -BODY:POSITION).
}

// A estrela: o corpo no topo da hierarquia (o nome muda com os mods de planetas).
FUNCTION star {
  LOCAL b IS BODY.
  UNTIL NOT b:HASBODY { SET b TO b:BODY. }
  RETURN b.
}

FUNCTION updSteer {
  IF steerMode = "prograde" { SET steerDir TO SHIP:PROGRADE. }
  ELSE IF steerMode = "retrograde" { SET steerDir TO SHIP:RETROGRADE. }
  ELSE IF steerMode = "normal" { SET steerDir TO normalVec(). }
  ELSE IF steerMode = "antinormal" { SET steerDir TO -normalVec(). }
  ELSE IF steerMode = "radial_out" { SET steerDir TO -BODY:POSITION. }
  ELSE IF steerMode = "radial_in" { SET steerDir TO BODY:POSITION. }
  ELSE IF steerMode = "up" { SET steerDir TO SHIP:UP. }
  ELSE IF steerMode = "heading" { SET steerDir TO HEADING(steerHdg, steerPitch). }
  ELSE IF steerMode = "hold" { SET steerDir TO holdDir. }
  ELSE IF steerMode = "sun" { SET steerDir TO star():POSITION. }
  ELSE IF steerMode = "target" AND HASTARGET { SET steerDir TO TARGET:POSITION. }
}

// Separa o estágio quando um motor apaga ou acaba o empuxo.
FUNCTION autoStage {
  IF NOT STAGE:READY OR STAGE:NUMBER = 0 { RETURN. }
  LOCAL e IS LIST().
  LIST ENGINES IN e.
  LOCAL need IS SHIP:MAXTHRUST = 0.
  FOR x IN e {
    IF x:IGNITION AND x:FLAMEOUT { SET need TO TRUE. }
  }
  IF need {
    STAGE.
    out("E Estágio separado (" + STAGE:NUMBER + " restantes)").
  }
}

FUNCTION makeCirc {
  PARAMETER where.
  LOCAL t IS TIME:SECONDS + ETA:APOAPSIS.
  LOCAL r IS BODY:RADIUS + APOAPSIS.
  IF where = "pe" {
    SET t TO TIME:SECONDS + ETA:PERIAPSIS.
    SET r TO BODY:RADIUS + PERIAPSIS.
  }
  LOCAL vNow IS VELOCITYAT(SHIP, t):ORBIT:MAG.
  LOCAL vCirc IS SQRT(BODY:MU / r).
  ADD NODE(t, 0, 0, vCirc - vNow).
  out("E Manobra de circularização criada: " + ROUND(vCirc - vNow, 1) + " m/s").
}

FUNCTION startAscent {
  PARAMETER apo, hdg.
  SET ascentApo TO apo.
  SET ascentHdg TO hdg.
  SET mode TO "ascent".
  SET steerDir TO HEADING(hdg, 90).
  steerOn().
  SET thr TO 1.
  throttleOn().
  IF SHIP:STATUS = "PRELAUNCH" OR SHIP:STATUS = "LANDED" {
    STAGE.
  }
  out("E Subida automática: apoastro " + ROUND(apo / 1000, 1) + " km, rumo " + hdg).
}

FUNCTION autopilot {
  IF mode = "ascent" {
    autoStage().
    LOCAL turnEnd IS 45000.
    IF BODY:ATM:EXISTS { SET turnEnd TO BODY:ATM:HEIGHT * 0.65. } ELSE { SET turnEnd TO 10000. }
    LOCAL pitch IS 90.
    IF SHIP:VELOCITY:SURFACE:MAG > 60 {
      SET pitch TO MAX(0, 90 - 90 * SQRT(MIN(1, ALTITUDE / turnEnd))).
    }
    SET steerDir TO HEADING(ascentHdg, pitch).
    IF APOAPSIS >= ascentApo {
      SET thr TO 0.
      SET mode TO "coast".
      out("E Corte do motor: apoastro " + ROUND(APOAPSIS / 1000, 1) + " km").
    }
  } ELSE IF mode = "coast" {
    SET steerDir TO SHIP:PROGRADE.
    LOCAL top IS 0.
    IF BODY:ATM:EXISTS { SET top TO BODY:ATM:HEIGHT. }
    // O arrasto rouba apoastro: repõe até sair da atmosfera.
    IF APOAPSIS < ascentApo * 0.995 AND ALTITUDE < top { SET thr TO 0.3. } ELSE { SET thr TO 0. }
    IF ALTITUDE > top {
      SET thr TO 0.
      makeCirc("ap").
      SET mode TO "node".
    }
  } ELSE IF mode = "node" {
    IF NOT HASNODE {
      SET mode TO "manual".
      release().
      RETURN.
    }
    LOCAL nd IS NEXTNODE.
    LOCAL bv IS nd:BURNVECTOR.
    steerOn().
    throttleOn().
    SET steerDir TO bv.
    LOCAL acc IS SHIP:AVAILABLETHRUST / SHIP:MASS.
    IF acc <= 0 AND NOT burning {
      out("E Sem empuxo disponível para a manobra").
      SET mode TO "manual".
      release().
      RETURN.
    }
    IF acc <= 0 {
      // O estágio acabou no meio da queima: separa e segue.
      autoStage().
      RETURN.
    }
    LOCAL bt IS nd:DELTAV:MAG / acc.
    IF NOT burning {
      SET thr TO 0.
      IF nd:ETA <= bt / 2 AND VANG(SHIP:FACING:FOREVECTOR, bv) < 3 {
        SET burning TO TRUE.
        SET burnVec0 TO bv.
        out("E Ignição: manobra de " + ROUND(nd:DELTAV:MAG, 1) + " m/s").
      }
    } ELSE {
      autoStage().
      SET thr TO MIN(1, MAX(0.02, nd:DELTAV:MAG / acc)).
      IF VDOT(burnVec0, bv) < 0 OR nd:DELTAV:MAG < 0.1 {
        SET thr TO 0.
        REMOVE nd.
        SET burning TO FALSE.
        SET mode TO "manual".
        release().
        out("E Corte: manobra concluída").
      }
    }
  } ELSE IF steerLocked {
    updSteer().
  }
  IF STAGE:NUMBER <> lastStage {
    SET lastStage TO STAGE:NUMBER.
  }
}

// ── Comandos ──

FUNCTION num {
  PARAMETER p, i, d.
  IF p:LENGTH > i { RETURN p[i]:REPLACE(",", "."):TONUMBER(d). }
  RETURN d.
}

FUNCTION word {
  PARAMETER p, i, d.
  IF p:LENGTH > i { RETURN p[i]:TOLOWER. }
  RETURN d.
}

FUNCTION doCmd {
  PARAMETER s, p.
  LOCAL c IS p[1]:TOLOWER.
  LOCAL ok IS TRUE.
  LOCAL msg IS c.
  IF c = "stage" {
    IF STAGE:READY { STAGE. } ELSE { SET ok TO FALSE. SET msg TO "estágio não está pronto". }
  } ELSE IF c = "throttle" {
    IF thrLocked { UNLOCK THROTTLE. SET thrLocked TO FALSE. }
    SET SHIP:CONTROL:PILOTMAINTHROTTLE TO MAX(0, MIN(1, num(p, 2, 0))).
  } ELSE IF c = "sas" {
    IF word(p, 2, "on") = "on" { release(). SAS ON. } ELSE { SAS OFF. }
  } ELSE IF c = "rcs" {
    IF word(p, 2, "on") = "on" { RCS ON. } ELSE { RCS OFF. }
  } ELSE IF c = "gear" {
    TOGGLE GEAR.
  } ELSE IF c = "lights" {
    TOGGLE LIGHTS.
  } ELSE IF c = "brakes" {
    TOGGLE BRAKES.
  } ELSE IF c = "abort" {
    SET mode TO "manual".
    release().
    ABORT ON.
  } ELSE IF c = "ag" {
    LOCAL n IS num(p, 2, 0).
    IF n = 1 { TOGGLE AG1. }
    ELSE IF n = 2 { TOGGLE AG2. }
    ELSE IF n = 3 { TOGGLE AG3. }
    ELSE IF n = 4 { TOGGLE AG4. }
    ELSE IF n = 5 { TOGGLE AG5. }
    ELSE IF n = 6 { TOGGLE AG6. }
    ELSE IF n = 7 { TOGGLE AG7. }
    ELSE IF n = 8 { TOGGLE AG8. }
    ELSE IF n = 9 { TOGGLE AG9. }
    ELSE IF n = 10 { TOGGLE AG10. }
    ELSE { SET ok TO FALSE. SET msg TO "grupo inválido". }
  } ELSE IF c = "chutes" {
    CHUTESSAFE ON.
  } ELSE IF c = "panels" {
    IF word(p, 2, "on") = "on" { PANELS ON. } ELSE { PANELS OFF. }
  } ELSE IF c = "steer" {
    LOCAL m IS word(p, 2, "free").
    IF m = "free" {
      release().
    } ELSE {
      SET steerMode TO m.
      SET holdDir TO SHIP:FACING.
      SET steerHdg TO num(p, 3, 90).
      SET steerPitch TO num(p, 4, 0).
      SET mode TO "manual".
      updSteer().
      steerOn().
    }
  } ELSE IF c = "warp" {
    SET WARP TO MAX(0, MIN(7, num(p, 2, 0))).
  } ELSE IF c = "node" {
    ADD NODE(TIME:SECONDS + num(p, 2, 60), num(p, 3, 0), num(p, 4, 0), num(p, 5, 0)).
  } ELSE IF c = "delnodes" {
    FOR n IN ALLNODES { REMOVE n. }
  } ELSE IF c = "exec" {
    IF HASNODE { SET mode TO "node". SET burning TO FALSE. } ELSE { SET ok TO FALSE. SET msg TO "não há manobra". }
  } ELSE IF c = "circ" {
    makeCirc(word(p, 2, "ap")).
  } ELSE IF c = "launch" {
    startAscent(num(p, 2, 80) * 1000, num(p, 3, 90)).
  } ELSE IF c = "stop" {
    SET mode TO "manual".
    SET thr TO 0.
    release().
  } ELSE IF c = "target" {
    // Alvo inexistente pararia o script: procura antes.
    LOCAL nm IS "".
    IF p:LENGTH > 2 { SET nm TO p[2]:REPLACE("_", " "). }
    LOCAL found IS FALSE.
    IF nm = "" {
      SET found TO FALSE.
    } ELSE IF BODYEXISTS(nm) {
      SET TARGET TO BODY(nm).
      SET found TO TRUE.
    } ELSE {
      LOCAL tl IS LIST().
      LIST TARGETS IN tl.
      FOR x IN tl {
        IF NOT found AND x:NAME = nm {
          SET TARGET TO x.
          SET found TO TRUE.
        }
      }
    }
    IF NOT found { SET ok TO FALSE. SET msg TO "alvo não encontrado". }
  } ELSE {
    SET ok TO FALSE.
    SET msg TO "comando desconhecido".
  }
  out("R " + s + " " + ok + " " + msg).
}

// Lê o arquivo de comandos; `doRun` falso só marca o que já existe como lido.
FUNCTION readCommands {
  PARAMETER doRun.
  IF NOT linked() OR NOT EXISTS(cmdf) { RETURN. }
  LOCAL txt IS OPEN(cmdf):READALL:STRING.
  FOR line IN txt:SPLIT(CHAR(10)) {
    LOCAL p IS line:TRIM:SPLIT(" ").
    IF p:LENGTH >= 2 {
      IF p[0] = "S" {
        // Sessão nova do Jaci: os números recomeçam.
        IF p[1] <> session { SET session TO p[1]. SET lastSeq TO -1. }
      } ELSE {
        LOCAL s IS p[0]:TONUMBER(-1).
        IF s > lastSeq {
          SET lastSeq TO s.
          IF doRun { doCmd(s, p). }
        }
      }
    }
  }
}

IF linked() AND EXISTS(tmf) { DELETEPATH(tmf). }
header().
readCommands(FALSE).
out("E Ponte do Jaci ativa no kOS").

UNTIL FALSE {
  SET tick TO tick + 1.
  IF BODY:NAME <> lastBody { bodyLine(). }
  IF MOD(tick, 10) = 0 { readCommands(TRUE). }
  autopilot().
  IF MOD(tick, 10) = 5 {
    telemetry().
    rotate().
  }
  WAIT 0.
}
