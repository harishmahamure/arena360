import { FormPage } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { useNavigate } from 'react-router-dom';
import { GameForm } from '../../../containers/games/GameForm';
import { addGame, type GamePayload } from '../../../services/game/add';

export default function GameNewPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });

  function handleSubmit(payload: GamePayload) {
    void run(async () => {
      await addGame(payload);
      setTimeout(() => navigate('/games'), 1200);
    });
  }

  return (
    <FormPage
      title="Add New Game"
      description="Upload branding assets and configure how this game appears on the kiosk."
      backTo="/games"
      backLabel="Back to games"
      breadcrumbs={[{ label: 'Games', to: '/games' }, { label: 'New game' }]}
    >
      <GameForm
        submitLabel="Create Game"
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Game created"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to create game'}
        onSubmit={handleSubmit}
        onCancel={() => navigate('/games')}
      />
    </FormPage>
  );
}
