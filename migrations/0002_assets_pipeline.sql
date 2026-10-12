ALTER TABLE assets ADD COLUMN processing_status text NOT NULL DEFAULT 'ready' CHECK(processing_status IN ('processing','ready','failed'));
ALTER TABLE assets ADD COLUMN processing_error text;
ALTER TABLE assets ADD COLUMN storage_key text;
ALTER TABLE assets ADD COLUMN lod1_key text;
ALTER TABLE assets ADD COLUMN lod2_key text;
ALTER TABLE assets ADD COLUMN thumb_key text;
INSERT INTO categories(slug,name,segment) VALUES
 ('escritorio','Escritório','escritorio'),('quarto','Quarto','quarto'),('oficina','Oficina','oficina'),
 ('consultorio','Consultório','consultorio'),('clinica','Clínica','clinica'),('estudio','Estúdio','estudio');
