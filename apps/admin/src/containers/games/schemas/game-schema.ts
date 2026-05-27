import { optionalString, stringWithLength, validationMessages } from '@gaming-cafe/utils';
import * as yup from 'yup';

export const GameCategoryValues = {
  ACTION: 'action',
  ADVENTURE: 'adventure',
  RPG: 'rpg',
  SPORTS: 'sports',
  RACING: 'racing',
  SIMULATION: 'simulation',
  PUZZLE: 'puzzle',
  STRATEGY: 'strategy',
  OTHER: 'other',
} as const;

export type GameCategoryType = (typeof GameCategoryValues)[keyof typeof GameCategoryValues];

export const gameCategoryOptions = [
  { value: GameCategoryValues.ACTION, label: 'Action' },
  { value: GameCategoryValues.ADVENTURE, label: 'Adventure' },
  { value: GameCategoryValues.RPG, label: 'RPG' },
  { value: GameCategoryValues.SPORTS, label: 'Sports' },
  { value: GameCategoryValues.RACING, label: 'Racing' },
  { value: GameCategoryValues.SIMULATION, label: 'Simulation' },
  { value: GameCategoryValues.PUZZLE, label: 'Puzzle' },
  { value: GameCategoryValues.STRATEGY, label: 'Strategy' },
  { value: GameCategoryValues.OTHER, label: 'Other' },
];

export const GamePlatformValues = {
  PC: 'PC',
  PLAYSTATION_4: 'PlayStation 4',
  PLAYSTATION_5: 'PlayStation 5',
  XBOX_ONE: 'Xbox One',
  XBOX_SERIES: 'Xbox Series X/S',
  NINTENDO_SWITCH: 'Nintendo Switch',
  MOBILE: 'Mobile',
  VR: 'VR',
  ARCADE: 'Arcade',
} as const;

export type GamePlatformType = (typeof GamePlatformValues)[keyof typeof GamePlatformValues];

export const gamePlatformOptions = [
  { value: GamePlatformValues.PC, label: 'PC' },
  { value: GamePlatformValues.PLAYSTATION_4, label: 'PlayStation 4' },
  { value: GamePlatformValues.PLAYSTATION_5, label: 'PlayStation 5' },
  { value: GamePlatformValues.XBOX_ONE, label: 'Xbox One' },
  { value: GamePlatformValues.XBOX_SERIES, label: 'Xbox Series X/S' },
  { value: GamePlatformValues.NINTENDO_SWITCH, label: 'Nintendo Switch' },
  { value: GamePlatformValues.MOBILE, label: 'Mobile' },
  { value: GamePlatformValues.VR, label: 'VR' },
  { value: GamePlatformValues.ARCADE, label: 'Arcade' },
];

export const AgeRatingValues = {
  EVERYONE: 'E for Everyone',
  EVERYONE_10_PLUS: 'E10+ Everyone 10+',
  TEEN: 'Teen',
  MATURE: 'Mature 17+',
  ADULTS_ONLY: 'Adults Only 18+',
  RATING_PENDING: 'Rating Pending',
  PEGI_3: 'PEGI 3',
  PEGI_7: 'PEGI 7',
  PEGI_12: 'PEGI 12',
  PEGI_16: 'PEGI 16',
  PEGI_18: 'PEGI 18',
} as const;

export type AgeRatingType = (typeof AgeRatingValues)[keyof typeof AgeRatingValues];

export const ageRatingOptions = [
  { value: AgeRatingValues.EVERYONE, label: 'E for Everyone' },
  { value: AgeRatingValues.EVERYONE_10_PLUS, label: 'E10+ Everyone 10+' },
  { value: AgeRatingValues.TEEN, label: 'Teen' },
  { value: AgeRatingValues.MATURE, label: 'Mature 17+' },
  { value: AgeRatingValues.ADULTS_ONLY, label: 'Adults Only 18+' },
  { value: AgeRatingValues.RATING_PENDING, label: 'Rating Pending' },
  { value: AgeRatingValues.PEGI_3, label: 'PEGI 3' },
  { value: AgeRatingValues.PEGI_7, label: 'PEGI 7' },
  { value: AgeRatingValues.PEGI_12, label: 'PEGI 12' },
  { value: AgeRatingValues.PEGI_16, label: 'PEGI 16' },
  { value: AgeRatingValues.PEGI_18, label: 'PEGI 18' },
];

export const createGameSchema = yup.object({
  title: stringWithLength('Game title', undefined, 200, true),

  description: optionalString(),

  genre: yup.string().max(100, validationMessages.max('Genre', 100)).optional().nullable(),

  category: yup
    .string()
    .oneOf(Object.values(GameCategoryValues), 'Please select a valid category')
    .optional()
    .nullable(),

  platform: yup
    .string()
    .oneOf(Object.values(GamePlatformValues), 'Please select a valid platform')
    .optional()
    .nullable(),

  isActive: yup.boolean().optional().default(true),

  imageUrl: yup.string().url('Please enter a valid URL').optional().nullable(),

  videoUrl: yup.string().url('Please enter a valid URL').optional().nullable(),

  trailerUrl: yup.string().url('Please enter a valid URL').optional().nullable(),

  developer: optionalString(),

  publisher: optionalString(),

  releaseDate: yup
    .date()
    .optional()
    .nullable()
    .transform((value, originalValue) => {
      if (originalValue === '' || originalValue === null) return null;
      return value;
    }),

  isMultiplayer: yup.boolean().optional().default(false),

  tags: yup.string().optional().nullable(),

  ageRating: yup
    .string()
    .oneOf(Object.values(AgeRatingValues), 'Please select a valid age rating')
    .optional()
    .nullable(),

  minPlayers: yup
    .number()
    .integer('Minimum players must be a whole number')
    .min(1, 'Minimum players must be at least 1')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value)),

  maxPlayers: yup
    .number()
    .integer('Maximum players must be a whole number')
    .min(1, 'Maximum players must be at least 1')
    .optional()
    .nullable()
    .transform((value) => (Number.isNaN(value) ? undefined : value))
    .test(
      'max-greater-than-min',
      'Maximum players must be greater than or equal to minimum players',
      function (value) {
        const { minPlayers } = this.parent;
        if (value && minPlayers && value < minPlayers) {
          return false;
        }
        return true;
      },
    ),
});

export type CreateGameFormData = yup.InferType<typeof createGameSchema>;

export const createGameDefaultValues: CreateGameFormData = {
  title: '',
  description: '',
  genre: '',
  category: undefined,
  platform: undefined,
  isActive: true,
  imageUrl: '',
  videoUrl: '',
  trailerUrl: '',
  developer: '',
  publisher: '',
  releaseDate: null,
  isMultiplayer: false,
  tags: '',
  ageRating: undefined,
  minPlayers: 1,
  maxPlayers: 1,
};
