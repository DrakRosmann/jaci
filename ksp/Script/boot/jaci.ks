// Boot do Jaci: inicia a ponte de telemetria e comandos ao carregar a nave.
WAIT UNTIL SHIP:UNPACKED.
IF HOMECONNECTION:ISCONNECTED {
  RUNPATH("0:/jaci/jaci.ks").
} ELSE {
  PRINT "Jaci: sem conexão com o KSC; rode RUNPATH(" + CHAR(34) + "0:/jaci/jaci.ks" + CHAR(34) + ") depois.".
}
