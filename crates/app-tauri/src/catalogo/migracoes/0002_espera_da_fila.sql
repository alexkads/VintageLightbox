-- A fila de envios em segundo plano (etapa D): quando uma linha pode ser
-- tentada de novo, em segundos desde a época. Falha de rede e 5xx esperam cada
-- vez mais, sem martelar o servidor nem gastar a bateria do balcão.
ALTER TABLE fila ADD COLUMN tentar_depois_de INTEGER NOT NULL DEFAULT 0;

CREATE INDEX fila_a_tentar ON fila (estado, tentar_depois_de, id);
