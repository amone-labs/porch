import { Section } from "../../components/ui";
import { UpdateRow } from "../../components/UpdateRow";
import { useT } from "../../i18n";
import { Row } from "./Row";

export function AboutTab() {
  const t = useT();
  return (
    <Section title={t.settings.app}>
      <Row label={t.settings.version}>
        <UpdateRow />
      </Row>
    </Section>
  );
}
